//! OCPP 1.6 charging profile storage and composite-limit evaluation.
//!
//! This models how a real charge point interprets `SetChargingProfile`
//! commands and derives an effective power limit, faithfully ported from the
//! EVerest reference implementation (libocpp v1.6,
//! `lib/everest/ocpp/lib/ocpp/v16/profile.cpp` + `smart_charging.cpp`).
//!
//! Simplifications for the game: a charger has a single connector, so
//! `connectorId` 0 and 1 refer to the same physical unit.

use std::collections::{BTreeMap, HashMap};

use bevy::prelude::*;
use chrono::{DateTime, Duration, Utc};
use rust_decimal::prelude::ToPrimitive;

use super::types::{
    ChargingProfile, ChargingProfileKindType, ChargingProfilePurposeType, ChargingRateUnitType,
    ChargingSchedulePeriod, RecurrencyKindType,
};

/// Nominal line voltage per phase used to convert Amp limits to Watts.
/// Matches EVerest's `LOW_VOLTAGE` constant.
pub const SUPPLY_VOLTAGE: f32 = 230.0;

/// Default number of phases assumed when a schedule period omits `numberPhases`.
/// Matches EVerest's `DEFAULT_AND_MAX_NUMBER_PHASES`.
pub const DEFAULT_PHASES: i32 = 3;

/// Recurrence interval for a `Daily` recurring profile, in seconds.
const DAILY_SECS: i64 = 86_400;
/// Recurrence interval for a `Weekly` recurring profile, in seconds.
const WEEKLY_SECS: i64 = 604_800;

/// Charging profiles installed on a single charger, grouped by purpose.
/// Each map is keyed by `stackLevel`; inserting at an existing stack level
/// replaces the previous profile (per OCPP replacement rules).
#[derive(Default, Clone, Debug)]
pub struct ChargerProfiles {
    pub charge_point_max: BTreeMap<u32, ChargingProfile>,
    pub tx_default: BTreeMap<u32, ChargingProfile>,
    pub tx: BTreeMap<u32, ChargingProfile>,
}

impl ChargerProfiles {
    /// Insert a validated profile, applying OCPP replacement rules:
    /// first remove any existing profile with the same `chargingProfileId`
    /// (across all purposes), then insert at its `(purpose, stackLevel)` slot.
    fn insert(&mut self, profile: ChargingProfile) {
        self.remove_by_id(profile.charging_profile_id);
        let map = self.map_for_mut(&profile.charging_profile_purpose);
        map.insert(profile.stack_level, profile);
    }

    fn map_for_mut(
        &mut self,
        purpose: &ChargingProfilePurposeType,
    ) -> &mut BTreeMap<u32, ChargingProfile> {
        match purpose {
            ChargingProfilePurposeType::ChargePointMaxProfile => &mut self.charge_point_max,
            ChargingProfilePurposeType::TxDefaultProfile => &mut self.tx_default,
            ChargingProfilePurposeType::TxProfile => &mut self.tx,
        }
    }

    fn remove_by_id(&mut self, id: i32) -> bool {
        let before = self.total_count();
        self.charge_point_max
            .retain(|_, p| p.charging_profile_id != id);
        self.tx_default.retain(|_, p| p.charging_profile_id != id);
        self.tx.retain(|_, p| p.charging_profile_id != id);
        before != self.total_count()
    }

    fn total_count(&self) -> usize {
        self.charge_point_max.len() + self.tx_default.len() + self.tx.len()
    }

    fn is_empty(&self) -> bool {
        self.total_count() == 0
    }

    /// Clear all `TxProfile` entries (called when a transaction ends).
    fn clear_tx(&mut self) {
        self.tx.clear();
    }
}

/// Resource holding charging profiles for every charger, keyed by charger id.
#[derive(Resource, Default)]
pub struct ChargingProfileStore {
    pub chargers: HashMap<String, ChargerProfiles>,
}

/// A single stored charging profile, summarized for read-only UI display.
#[derive(Debug, Clone)]
pub struct ActiveProfileRow {
    pub charger_id: String,
    pub profile_id: i32,
    pub purpose: ChargingProfilePurposeType,
    pub stack_level: u32,
    pub kind: ChargingProfileKindType,
    pub unit: ChargingRateUnitType,
    /// The profile's currently-active limit converted to kW, or `None` if the
    /// profile is outside its validity/schedule window right now.
    pub active_limit_kw: Option<f32>,
}

/// Evaluate each charger's composite charging-profile limit at the current game
/// time and write it to `Charger::ocpp_limit_kw`. Runs before
/// `power_dispatch_system` so the cap flows into power allocation the same tick.
pub fn apply_charging_profiles_system(
    mut chargers: Query<(Entity, &mut crate::components::charger::Charger)>,
    store: Res<ChargingProfileStore>,
    queue: Res<super::queue::OcppMessageQueue>,
    game_clock: Res<crate::resources::GameClock>,
) {
    let now = queue.game_time_to_utc(game_clock.total_game_time);
    for (entity, mut charger) in &mut chargers {
        let tx_start = queue
            .charger_state
            .get(&entity)
            .and_then(|state| state.tx_start_total_game_time)
            .map(|t| queue.game_time_to_utc(t));
        let limit = store.composite_limit_kw(&charger.id, now, tx_start);
        // Avoid needless change-detection churn when the limit is unchanged.
        if charger.ocpp_limit_kw != limit {
            charger.ocpp_limit_kw = limit;
        }
    }
}

/// Outcome of validating and storing a `SetChargingProfile` request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetProfileResult {
    Accepted,
    /// Profile failed OCPP validation; `reason` is a short static description.
    Rejected {
        reason: &'static str,
    },
}

impl ChargingProfileStore {
    /// Validate and store a profile for a charger. Returns whether the request
    /// is accepted, following EVerest's `validate_profile`/`validate_schedule`.
    ///
    /// `active_tx_id` is the charger's current transaction id, if any (used to
    /// validate `TxProfile` requests).
    pub fn set_profile(
        &mut self,
        charger_id: &str,
        connector_id: i32,
        profile: ChargingProfile,
        active_tx_id: Option<i32>,
    ) -> SetProfileResult {
        if let Err(reason) = validate_profile(connector_id, &profile, active_tx_id) {
            return SetProfileResult::Rejected { reason };
        }
        self.chargers
            .entry(charger_id.to_string())
            .or_default()
            .insert(profile);
        SetProfileResult::Accepted
    }

    /// Clear matching profiles for a charger, per OCPP `ClearChargingProfile`
    /// matching rules. Returns `true` if at least one profile was removed.
    ///
    /// - `id` present: match by `chargingProfileId` only.
    /// - otherwise: match on the provided `connector_id`, `stack_level` and
    ///   `purpose` (absent fields act as wildcards).
    pub fn clear_profiles(
        &mut self,
        charger_id: &str,
        id: Option<i32>,
        connector_id: Option<i32>,
        purpose: Option<ChargingProfilePurposeType>,
        stack_level: Option<i32>,
    ) -> bool {
        let Some(profiles) = self.chargers.get_mut(charger_id) else {
            return false;
        };

        // Single connector: a `connectorId` filter of 0 or 1 matches this unit;
        // any other explicit connector id matches nothing.
        if let Some(cid) = connector_id
            && cid > 1
        {
            return false;
        }

        let before = profiles.total_count();

        if let Some(id) = id {
            profiles.remove_by_id(id);
        } else {
            let matches = |p: &ChargingProfile| {
                purpose
                    .as_ref()
                    .map(|want| &p.charging_profile_purpose == want)
                    .unwrap_or(true)
                    && stack_level
                        .map(|want| p.stack_level as i32 == want)
                        .unwrap_or(true)
            };
            profiles.charge_point_max.retain(|_, p| !matches(p));
            profiles.tx_default.retain(|_, p| !matches(p));
            profiles.tx.retain(|_, p| !matches(p));
        }

        let removed = before != profiles.total_count();
        if profiles.is_empty() {
            self.chargers.remove(charger_id);
        }
        removed
    }

    /// Clear all `TxProfile`s for a charger (invoked when its transaction ends).
    pub fn clear_tx(&mut self, charger_id: &str) {
        if let Some(profiles) = self.chargers.get_mut(charger_id) {
            profiles.clear_tx();
            if profiles.is_empty() {
                self.chargers.remove(charger_id);
            }
        }
    }

    /// Compute the composite power limit (kW) applicable to a charger at `now`,
    /// or `None` when no profile constrains it (treated as unlimited).
    ///
    /// `tx_start` is the current transaction's start time, used to anchor
    /// `Relative` profiles.
    pub fn composite_limit_kw(
        &self,
        charger_id: &str,
        now: DateTime<Utc>,
        tx_start: Option<DateTime<Utc>>,
    ) -> Option<f32> {
        let profiles = self.chargers.get(charger_id)?;
        composite_limit_kw(profiles, now, tx_start)
    }

    /// `true` when no charger has any stored charging profile.
    pub fn is_empty(&self) -> bool {
        self.chargers.values().all(|p| p.is_empty())
    }

    /// Summarize every stored profile for a charger (all purposes, ordered by
    /// stack level within each purpose), for read-only UI display. Each row's
    /// `active_limit_kw` reflects the currently-active schedule period at `now`.
    pub fn profile_rows(
        &self,
        charger_id: &str,
        now: DateTime<Utc>,
        tx_start: Option<DateTime<Utc>>,
    ) -> Vec<ActiveProfileRow> {
        let Some(profiles) = self.chargers.get(charger_id) else {
            return Vec::new();
        };

        let mut rows = Vec::new();
        // ChargePointMax first (site cap), then TxDefault, then Tx.
        for map in [
            &profiles.charge_point_max,
            &profiles.tx_default,
            &profiles.tx,
        ] {
            for profile in map.values() {
                let active_limit_kw =
                    profile_active_limit(profile, now, tx_start).map(|(limit, phases)| {
                        to_kw(limit, &profile.charging_schedule.charging_rate_unit, phases)
                    });
                rows.push(ActiveProfileRow {
                    charger_id: charger_id.to_string(),
                    profile_id: profile.charging_profile_id,
                    purpose: profile.charging_profile_purpose.clone(),
                    stack_level: profile.stack_level,
                    kind: profile.charging_profile_kind.clone(),
                    unit: profile.charging_schedule.charging_rate_unit.clone(),
                    active_limit_kw,
                });
            }
        }
        rows
    }

    /// Total number of stored profiles across all chargers (cheap change signal).
    pub fn total_profile_count(&self) -> usize {
        self.chargers.values().map(|p| p.total_count()).sum()
    }
}

/// Validate a profile against the OCPP 1.6 rules EVerest enforces.
fn validate_profile(
    connector_id: i32,
    profile: &ChargingProfile,
    active_tx_id: Option<i32>,
) -> Result<(), &'static str> {
    if connector_id < 0 {
        return Err("negative connectorId");
    }

    validate_schedule(profile)?;

    match profile.charging_profile_purpose {
        ChargingProfilePurposeType::ChargePointMaxProfile => {
            if connector_id != 0 {
                return Err("ChargePointMaxProfile must target connector 0");
            }
            if profile.charging_profile_kind == ChargingProfileKindType::Relative {
                return Err("ChargePointMaxProfile cannot be Relative");
            }
        }
        ChargingProfilePurposeType::TxProfile => {
            if connector_id == 0 {
                return Err("TxProfile must target a connector > 0");
            }
            match (profile.transaction_id, active_tx_id) {
                // A referenced transaction id must match the active transaction.
                (Some(pid), Some(active)) if pid != active => {
                    return Err("TxProfile transactionId mismatch");
                }
                // No active transaction at all: reject (real charge points
                // discard TxProfiles with no transaction on the connector).
                (_, None) => return Err("TxProfile without active transaction"),
                _ => {}
            }
        }
        ChargingProfilePurposeType::TxDefaultProfile => {}
    }

    match profile.charging_profile_kind {
        ChargingProfileKindType::Recurring => {
            if profile.recurrency_kind.is_none() {
                return Err("Recurring profile requires recurrencyKind");
            }
            if profile.charging_schedule.start_schedule.is_none() {
                return Err("Recurring profile requires startSchedule");
            }
        }
        ChargingProfileKindType::Absolute | ChargingProfileKindType::Relative => {}
    }

    Ok(())
}

/// Validate a profile's schedule (periods, limits, phases).
fn validate_schedule(profile: &ChargingProfile) -> Result<(), &'static str> {
    let schedule = &profile.charging_schedule;
    if schedule.charging_schedule_period.is_empty() {
        return Err("empty charging schedule");
    }
    // The first period must start at 0.
    if schedule.charging_schedule_period[0].start_period != 0 {
        return Err("first period startPeriod must be 0");
    }

    let mut last_start = -1;
    for period in &schedule.charging_schedule_period {
        // startPeriod must be monotonically increasing.
        if period.start_period <= last_start {
            return Err("non-monotonic startPeriod");
        }
        last_start = period.start_period;

        let limit = period.limit.to_f32().unwrap_or(f32::NAN);
        if !limit.is_finite() || limit < 0.0 {
            return Err("invalid period limit");
        }
        if let Some(phases) = period.number_phases
            && !(1..=DEFAULT_PHASES).contains(&phases)
        {
            return Err("numberPhases out of range");
        }
    }
    Ok(())
}

/// Convert a limit expressed in the profile's unit to kW.
fn to_kw(limit: f32, unit: &ChargingRateUnitType, phases: Option<i32>) -> f32 {
    match unit {
        ChargingRateUnitType::W => limit / 1000.0,
        ChargingRateUnitType::A => {
            let phases = phases.unwrap_or(DEFAULT_PHASES).max(1) as f32;
            limit * SUPPLY_VOLTAGE * phases / 1000.0
        }
    }
}

/// The composite limit (kW) at `now`, combining all purposes per OCPP rules:
/// `min(ChargePointMaxProfile, TxProfile or TxDefaultProfile)`.
fn composite_limit_kw(
    profiles: &ChargerProfiles,
    now: DateTime<Utc>,
    tx_start: Option<DateTime<Utc>>,
) -> Option<f32> {
    let cp_max = purpose_limit_kw(&profiles.charge_point_max, now, tx_start);
    let tx = purpose_limit_kw(&profiles.tx, now, tx_start);
    let tx_default = purpose_limit_kw(&profiles.tx_default, now, tx_start);

    // TxProfile overrides TxDefaultProfile for the duration of a transaction.
    let base = tx.or(tx_default);

    match (base, cp_max) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

/// Limit (kW) contributed by a single purpose at `now`. Higher stack levels win
/// where they define a period; gaps fall through to lower stack levels.
fn purpose_limit_kw(
    map: &BTreeMap<u32, ChargingProfile>,
    now: DateTime<Utc>,
    tx_start: Option<DateTime<Utc>>,
) -> Option<f32> {
    // BTreeMap iterates ascending; reverse to visit highest stack level first.
    for profile in map.values().rev() {
        if let Some((limit, phases)) = profile_active_limit(profile, now, tx_start) {
            let unit = &profile.charging_schedule.charging_rate_unit;
            return Some(to_kw(limit, unit, phases));
        }
    }
    None
}

/// The active limit (in the profile's own unit) and phase count at `now`, if the
/// profile currently applies. Returns `None` outside validity/schedule windows.
fn profile_active_limit(
    profile: &ChargingProfile,
    now: DateTime<Utc>,
    tx_start: Option<DateTime<Utc>>,
) -> Option<(f32, Option<i32>)> {
    if let Some(valid_from) = profile.valid_from
        && now < valid_from
    {
        return None;
    }
    if let Some(valid_to) = profile.valid_to
        && now >= valid_to
    {
        return None;
    }

    let schedule = &profile.charging_schedule;
    let duration_secs = schedule.duration.map(|d| d as i64);

    match profile.charging_profile_kind {
        ChargingProfileKindType::Absolute => {
            let anchor = schedule
                .start_schedule
                .or(profile.valid_from)
                .unwrap_or(now);
            let max_len = duration_secs.unwrap_or(i64::MAX);
            period_limit_at(
                schedule.charging_schedule_period.as_slice(),
                anchor,
                now,
                max_len,
            )
        }
        ChargingProfileKindType::Relative => {
            let anchor = tx_start.unwrap_or(now);
            let max_len = duration_secs.unwrap_or(i64::MAX);
            period_limit_at(
                schedule.charging_schedule_period.as_slice(),
                anchor,
                now,
                max_len,
            )
        }
        ChargingProfileKindType::Recurring => {
            let start_schedule = schedule.start_schedule?;
            let interval = match profile.recurrency_kind {
                Some(RecurrencyKindType::Weekly) => WEEKLY_SECS,
                _ => DAILY_SECS,
            };
            if now < start_schedule {
                return None;
            }
            let elapsed_total = (now - start_schedule).num_seconds();
            let n = elapsed_total / interval;
            let anchor = start_schedule + Duration::seconds(n * interval);
            // A recurring schedule is only active for the shorter of its
            // declared duration and the recurrence interval; the remainder of
            // the period is a gap (no limit).
            let max_len = duration_secs.map(|d| d.min(interval)).unwrap_or(interval);
            period_limit_at(
                schedule.charging_schedule_period.as_slice(),
                anchor,
                now,
                max_len,
            )
        }
    }
}

/// Find the schedule period covering `now` relative to `anchor` and return its
/// limit and phase count. `max_len` bounds the schedule length in seconds.
fn period_limit_at(
    periods: &[ChargingSchedulePeriod],
    anchor: DateTime<Utc>,
    now: DateTime<Utc>,
    max_len: i64,
) -> Option<(f32, Option<i32>)> {
    let elapsed = (now - anchor).num_seconds();
    if elapsed < 0 || elapsed >= max_len {
        return None;
    }

    let mut chosen: Option<&ChargingSchedulePeriod> = None;
    for (i, period) in periods.iter().enumerate() {
        let start = period.start_period as i64;
        if start > elapsed {
            break;
        }
        let next_end = periods
            .get(i + 1)
            .map(|p| p.start_period as i64)
            .unwrap_or(max_len)
            .min(max_len);
        if elapsed < next_end {
            chosen = Some(period);
        }
    }

    chosen.map(|p| (p.limit.to_f32().unwrap_or(0.0), p.number_phases))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    fn schedule_period(start: i32, limit: i64, phases: Option<i32>) -> ChargingSchedulePeriod {
        ChargingSchedulePeriod {
            start_period: start,
            limit: Decimal::from(limit),
            number_phases: phases,
        }
    }

    fn profile(
        id: i32,
        stack: u32,
        purpose: ChargingProfilePurposeType,
        kind: ChargingProfileKindType,
        unit: ChargingRateUnitType,
        periods: Vec<ChargingSchedulePeriod>,
    ) -> ChargingProfile {
        ChargingProfile {
            charging_profile_id: id,
            transaction_id: None,
            stack_level: stack,
            charging_profile_purpose: purpose,
            charging_profile_kind: kind,
            recurrency_kind: None,
            valid_from: None,
            valid_to: None,
            charging_schedule: crate::ocpp::types::ChargingSchedule {
                duration: None,
                start_schedule: Some(DateTime::<Utc>::UNIX_EPOCH),
                charging_rate_unit: unit,
                charging_schedule_period: periods,
                min_charging_rate: None,
            },
        }
    }

    #[test]
    fn watts_limit_converts_directly_to_kw() {
        let mut store = ChargingProfileStore::default();
        let p = profile(
            1,
            0,
            ChargingProfilePurposeType::TxDefaultProfile,
            ChargingProfileKindType::Absolute,
            ChargingRateUnitType::W,
            vec![schedule_period(0, 22_000, Some(3))],
        );
        assert_eq!(
            store.set_profile("c1", 1, p, None),
            SetProfileResult::Accepted
        );
        let now = DateTime::<Utc>::UNIX_EPOCH + Duration::seconds(10);
        let limit = store.composite_limit_kw("c1", now, None).unwrap();
        assert!((limit - 22.0).abs() < 1e-3);
    }

    #[test]
    fn profile_rows_report_active_and_inactive() {
        let mut store = ChargingProfileStore::default();
        assert!(store.is_empty());

        // Active window profile (valid always, 22 kW).
        store.set_profile(
            "c1",
            0,
            profile(
                1,
                0,
                ChargingProfilePurposeType::ChargePointMaxProfile,
                ChargingProfileKindType::Absolute,
                ChargingRateUnitType::W,
                vec![schedule_period(0, 22_000, Some(3))],
            ),
            None,
        );
        // Profile that only becomes valid later (currently inactive).
        let mut future = profile(
            2,
            0,
            ChargingProfilePurposeType::TxDefaultProfile,
            ChargingProfileKindType::Absolute,
            ChargingRateUnitType::W,
            vec![schedule_period(0, 5_000, Some(3))],
        );
        future.valid_from = Some(DateTime::<Utc>::UNIX_EPOCH + Duration::seconds(1_000));
        store.set_profile("c1", 0, future, None);

        assert!(!store.is_empty());
        assert_eq!(store.total_profile_count(), 2);

        let now = DateTime::<Utc>::UNIX_EPOCH + Duration::seconds(10);
        let rows = store.profile_rows("c1", now, None);
        assert_eq!(rows.len(), 2);

        let cp_max = rows.iter().find(|r| r.profile_id == 1).expect("cp max row");
        assert_eq!(
            cp_max.purpose,
            ChargingProfilePurposeType::ChargePointMaxProfile
        );
        assert!((cp_max.active_limit_kw.unwrap() - 22.0).abs() < 1e-3);

        let future_row = rows.iter().find(|r| r.profile_id == 2).expect("future row");
        assert!(future_row.active_limit_kw.is_none());

        // No profiles for an unknown charger.
        assert!(store.profile_rows("nope", now, None).is_empty());
    }

    #[test]
    fn amps_limit_converts_via_voltage_and_phases() {
        let mut store = ChargingProfileStore::default();
        let p = profile(
            1,
            0,
            ChargingProfilePurposeType::TxDefaultProfile,
            ChargingProfileKindType::Absolute,
            ChargingRateUnitType::A,
            vec![schedule_period(0, 32, Some(3))],
        );
        store.set_profile("c1", 1, p, None);
        let now = DateTime::<Utc>::UNIX_EPOCH + Duration::seconds(10);
        let limit = store.composite_limit_kw("c1", now, None).unwrap();
        // 32 A * 230 V * 3 phases = 22_080 W = 22.08 kW
        assert!((limit - 22.08).abs() < 1e-2);
    }

    #[test]
    fn higher_stack_level_wins_within_purpose() {
        let mut store = ChargingProfileStore::default();
        store.set_profile(
            "c1",
            1,
            profile(
                1,
                0,
                ChargingProfilePurposeType::TxDefaultProfile,
                ChargingProfileKindType::Absolute,
                ChargingRateUnitType::W,
                vec![schedule_period(0, 40_000, Some(3))],
            ),
            None,
        );
        store.set_profile(
            "c1",
            1,
            profile(
                2,
                5,
                ChargingProfilePurposeType::TxDefaultProfile,
                ChargingProfileKindType::Absolute,
                ChargingRateUnitType::W,
                vec![schedule_period(0, 10_000, Some(3))],
            ),
            None,
        );
        let now = DateTime::<Utc>::UNIX_EPOCH + Duration::seconds(10);
        let limit = store.composite_limit_kw("c1", now, None).unwrap();
        assert!((limit - 10.0).abs() < 1e-3);
    }

    #[test]
    fn charge_point_max_caps_tx_default() {
        let mut store = ChargingProfileStore::default();
        store.set_profile(
            "c1",
            1,
            profile(
                1,
                0,
                ChargingProfilePurposeType::TxDefaultProfile,
                ChargingProfileKindType::Absolute,
                ChargingRateUnitType::W,
                vec![schedule_period(0, 50_000, Some(3))],
            ),
            None,
        );
        store.set_profile(
            "c1",
            0,
            profile(
                2,
                0,
                ChargingProfilePurposeType::ChargePointMaxProfile,
                ChargingProfileKindType::Absolute,
                ChargingRateUnitType::W,
                vec![schedule_period(0, 20_000, Some(3))],
            ),
            None,
        );
        let now = DateTime::<Utc>::UNIX_EPOCH + Duration::seconds(10);
        let limit = store.composite_limit_kw("c1", now, None).unwrap();
        assert!((limit - 20.0).abs() < 1e-3);
    }

    #[test]
    fn tx_profile_overrides_tx_default() {
        let mut store = ChargingProfileStore::default();
        store.set_profile(
            "c1",
            1,
            profile(
                1,
                0,
                ChargingProfilePurposeType::TxDefaultProfile,
                ChargingProfileKindType::Absolute,
                ChargingRateUnitType::W,
                vec![schedule_period(0, 50_000, Some(3))],
            ),
            None,
        );
        // TxProfile requires an active transaction; supply id 7.
        let mut tx = profile(
            2,
            0,
            ChargingProfilePurposeType::TxProfile,
            ChargingProfileKindType::Absolute,
            ChargingRateUnitType::W,
            vec![schedule_period(0, 30_000, Some(3))],
        );
        tx.transaction_id = Some(7);
        assert_eq!(
            store.set_profile("c1", 1, tx, Some(7)),
            SetProfileResult::Accepted
        );
        let now = DateTime::<Utc>::UNIX_EPOCH + Duration::seconds(10);
        let limit = store.composite_limit_kw("c1", now, None).unwrap();
        assert!((limit - 30.0).abs() < 1e-3);
    }

    #[test]
    fn multi_period_schedule_selects_by_elapsed_time() {
        let mut store = ChargingProfileStore::default();
        store.set_profile(
            "c1",
            1,
            profile(
                1,
                0,
                ChargingProfilePurposeType::TxDefaultProfile,
                ChargingProfileKindType::Absolute,
                ChargingRateUnitType::W,
                vec![
                    schedule_period(0, 10_000, Some(3)),
                    schedule_period(100, 20_000, Some(3)),
                    schedule_period(200, 5_000, Some(3)),
                ],
            ),
            None,
        );
        let at = |secs| DateTime::<Utc>::UNIX_EPOCH + Duration::seconds(secs);
        assert!((store.composite_limit_kw("c1", at(50), None).unwrap() - 10.0).abs() < 1e-3);
        assert!((store.composite_limit_kw("c1", at(150), None).unwrap() - 20.0).abs() < 1e-3);
        assert!((store.composite_limit_kw("c1", at(250), None).unwrap() - 5.0).abs() < 1e-3);
    }

    #[test]
    fn relative_profile_anchors_to_transaction_start() {
        let mut store = ChargingProfileStore::default();
        let mut p = profile(
            1,
            0,
            ChargingProfilePurposeType::TxProfile,
            ChargingProfileKindType::Relative,
            ChargingRateUnitType::W,
            vec![
                schedule_period(0, 7_000, Some(1)),
                schedule_period(60, 3_000, Some(1)),
            ],
        );
        p.transaction_id = Some(1);
        p.charging_schedule.start_schedule = None;
        store.set_profile("c1", 1, p, Some(1));

        let tx_start = DateTime::<Utc>::UNIX_EPOCH + Duration::seconds(1_000);
        // 30s into the transaction -> first period (7 kW).
        let now1 = tx_start + Duration::seconds(30);
        assert!(
            (store
                .composite_limit_kw("c1", now1, Some(tx_start))
                .unwrap()
                - 7.0)
                .abs()
                < 1e-3
        );
        // 90s into the transaction -> second period (3 kW).
        let now2 = tx_start + Duration::seconds(90);
        assert!(
            (store
                .composite_limit_kw("c1", now2, Some(tx_start))
                .unwrap()
                - 3.0)
                .abs()
                < 1e-3
        );
    }

    #[test]
    fn recurring_daily_profile_wraps_across_days() {
        let mut store = ChargingProfileStore::default();
        let mut p = profile(
            1,
            0,
            ChargingProfilePurposeType::TxDefaultProfile,
            ChargingProfileKindType::Recurring,
            ChargingRateUnitType::W,
            vec![
                schedule_period(0, 5_000, Some(3)),
                schedule_period(3_600, 15_000, Some(3)),
            ],
        );
        p.recurrency_kind = Some(RecurrencyKindType::Daily);
        p.charging_schedule.start_schedule = Some(DateTime::<Utc>::UNIX_EPOCH);
        store.set_profile("c1", 1, p, None);

        // Day 3, 30 min in -> first period (5 kW).
        let now1 = DateTime::<Utc>::UNIX_EPOCH + Duration::seconds(3 * DAILY_SECS + 1_800);
        assert!((store.composite_limit_kw("c1", now1, None).unwrap() - 5.0).abs() < 1e-3);
        // Day 3, 90 min in -> second period (15 kW).
        let now2 = DateTime::<Utc>::UNIX_EPOCH + Duration::seconds(3 * DAILY_SECS + 5_400);
        assert!((store.composite_limit_kw("c1", now2, None).unwrap() - 15.0).abs() < 1e-3);
    }

    #[test]
    fn validity_window_excludes_expired_profiles() {
        let mut store = ChargingProfileStore::default();
        let mut p = profile(
            1,
            0,
            ChargingProfilePurposeType::TxDefaultProfile,
            ChargingProfileKindType::Absolute,
            ChargingRateUnitType::W,
            vec![schedule_period(0, 11_000, Some(3))],
        );
        p.valid_from = Some(DateTime::<Utc>::UNIX_EPOCH + Duration::seconds(100));
        p.valid_to = Some(DateTime::<Utc>::UNIX_EPOCH + Duration::seconds(200));
        store.set_profile("c1", 1, p, None);

        let before = DateTime::<Utc>::UNIX_EPOCH + Duration::seconds(50);
        let during = DateTime::<Utc>::UNIX_EPOCH + Duration::seconds(150);
        let after = DateTime::<Utc>::UNIX_EPOCH + Duration::seconds(250);
        assert!(store.composite_limit_kw("c1", before, None).is_none());
        assert!(store.composite_limit_kw("c1", during, None).is_some());
        assert!(store.composite_limit_kw("c1", after, None).is_none());
    }

    #[test]
    fn charge_point_max_on_connector_1_is_rejected() {
        let mut store = ChargingProfileStore::default();
        let p = profile(
            1,
            0,
            ChargingProfilePurposeType::ChargePointMaxProfile,
            ChargingProfileKindType::Absolute,
            ChargingRateUnitType::W,
            vec![schedule_period(0, 20_000, Some(3))],
        );
        assert_eq!(
            store.set_profile("c1", 1, p, None),
            SetProfileResult::Rejected {
                reason: "ChargePointMaxProfile must target connector 0"
            }
        );
    }

    #[test]
    fn tx_profile_without_transaction_is_rejected() {
        let mut store = ChargingProfileStore::default();
        let p = profile(
            1,
            0,
            ChargingProfilePurposeType::TxProfile,
            ChargingProfileKindType::Absolute,
            ChargingRateUnitType::W,
            vec![schedule_period(0, 20_000, Some(3))],
        );
        assert_eq!(
            store.set_profile("c1", 1, p, None),
            SetProfileResult::Rejected {
                reason: "TxProfile without active transaction"
            }
        );
    }

    #[test]
    fn clear_by_id_removes_matching_profile() {
        let mut store = ChargingProfileStore::default();
        store.set_profile(
            "c1",
            1,
            profile(
                42,
                0,
                ChargingProfilePurposeType::TxDefaultProfile,
                ChargingProfileKindType::Absolute,
                ChargingRateUnitType::W,
                vec![schedule_period(0, 20_000, Some(3))],
            ),
            None,
        );
        assert!(store.clear_profiles("c1", Some(42), None, None, None));
        let now = DateTime::<Utc>::UNIX_EPOCH + Duration::seconds(10);
        assert!(store.composite_limit_kw("c1", now, None).is_none());
    }
}

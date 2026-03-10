//! Technician resource for field repairs

use std::collections::VecDeque;

use bevy::prelude::*;

/// Randomly assigned at game start to select which sprite set to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TechnicianGender {
    Male,
    Female,
}

impl TechnicianGender {
    /// 50/50 coin flip.
    pub fn random() -> Self {
        if rand::random::<bool>() {
            Self::Female
        } else {
            Self::Male
        }
    }
}

/// Technician hourly rate for OpEx calculations
pub const TECHNICIAN_HOURLY_RATE: f32 = 150.0; // $/hour

/// Base travel time to reach site (game seconds)
pub const BASE_TRAVEL_TIME: f32 = 1800.0; // 30 minutes

/// Queued dispatch request for technician
#[derive(Debug, Clone)]
pub struct QueuedDispatch {
    pub request_id: crate::resources::RepairRequestId,
    pub charger_entity: Entity,
    pub charger_id: String,
    pub site_id: crate::resources::SiteId,
}

/// Technician status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TechStatus {
    /// Technician is idle and available for dispatch
    #[default]
    Idle,
    /// Technician is traveling to the site
    EnRoute,
    /// Technician reached the site but is waiting to visibly execute the repair
    WaitingAtSite,
    /// Technician entity spawned, walking to charger on-site
    WalkingOnSite,
    /// Technician is performing repair on-site
    Repairing,
    /// Technician finished repair and is walking to exit
    LeavingSite,
}

#[derive(Debug, Clone)]
pub enum TechnicianMode {
    Idle,
    EnRoute {
        request_id: crate::resources::RepairRequestId,
        charger_entity: Entity,
        site_id: crate::resources::SiteId,
        travel_remaining: f32,
        travel_total: f32,
        repair_remaining: f32,
    },
    WaitingAtSite {
        request_id: crate::resources::RepairRequestId,
        charger_entity: Entity,
        site_id: crate::resources::SiteId,
        repair_remaining: f32,
    },
    WalkingOnSite {
        request_id: crate::resources::RepairRequestId,
        charger_entity: Entity,
        site_id: crate::resources::SiteId,
        repair_remaining: f32,
    },
    Repairing {
        request_id: crate::resources::RepairRequestId,
        charger_entity: Entity,
        site_id: crate::resources::SiteId,
        repair_remaining: f32,
    },
    LeavingSite {
        site_id: crate::resources::SiteId,
    },
}

/// Technician state - single shared technician
#[derive(Resource, Debug, Clone)]
pub struct TechnicianState {
    /// Randomly chosen at game start; selects the sprite set.
    pub gender: TechnicianGender,
    /// Current site location (None = not at any site yet)
    pub current_site_id: Option<crate::resources::SiteId>,
    /// Total time spent on this job (for billing)
    pub job_time_elapsed: f32,
    /// Current technician mode, including the active request when one exists.
    pub mode: TechnicianMode,
    /// Queue of pending dispatch requests
    pub dispatch_queue: VecDeque<QueuedDispatch>,
}

impl Default for TechnicianState {
    fn default() -> Self {
        Self {
            gender: TechnicianGender::random(),
            current_site_id: None,
            job_time_elapsed: 0.0,
            mode: TechnicianMode::Idle,
            dispatch_queue: VecDeque::new(),
        }
    }
}

impl TechnicianState {
    pub fn status(&self) -> TechStatus {
        match self.mode {
            TechnicianMode::Idle => TechStatus::Idle,
            TechnicianMode::EnRoute { .. } => TechStatus::EnRoute,
            TechnicianMode::WaitingAtSite { .. } => TechStatus::WaitingAtSite,
            TechnicianMode::WalkingOnSite { .. } => TechStatus::WalkingOnSite,
            TechnicianMode::Repairing { .. } => TechStatus::Repairing,
            TechnicianMode::LeavingSite { .. } => TechStatus::LeavingSite,
        }
    }

    /// Check if technician is available for dispatch
    pub fn is_available(&self) -> bool {
        matches!(self.mode, TechnicianMode::Idle)
    }

    pub fn active_request_id(&self) -> Option<crate::resources::RepairRequestId> {
        match self.mode {
            TechnicianMode::EnRoute { request_id, .. }
            | TechnicianMode::WaitingAtSite { request_id, .. }
            | TechnicianMode::WalkingOnSite { request_id, .. }
            | TechnicianMode::Repairing { request_id, .. } => Some(request_id),
            TechnicianMode::Idle | TechnicianMode::LeavingSite { .. } => None,
        }
    }

    pub fn active_charger(&self) -> Option<Entity> {
        match self.mode {
            TechnicianMode::EnRoute { charger_entity, .. }
            | TechnicianMode::WaitingAtSite { charger_entity, .. }
            | TechnicianMode::WalkingOnSite { charger_entity, .. }
            | TechnicianMode::Repairing { charger_entity, .. } => Some(charger_entity),
            TechnicianMode::Idle | TechnicianMode::LeavingSite { .. } => None,
        }
    }

    pub fn destination_site_id(&self) -> Option<crate::resources::SiteId> {
        match self.mode {
            TechnicianMode::EnRoute { site_id, .. }
            | TechnicianMode::WaitingAtSite { site_id, .. }
            | TechnicianMode::WalkingOnSite { site_id, .. }
            | TechnicianMode::Repairing { site_id, .. } => Some(site_id),
            TechnicianMode::Idle | TechnicianMode::LeavingSite { .. } => None,
        }
    }

    pub fn leaving_site_id(&self) -> Option<crate::resources::SiteId> {
        match self.mode {
            TechnicianMode::LeavingSite { site_id } => Some(site_id),
            _ => None,
        }
    }

    pub fn active_site_id(&self) -> Option<crate::resources::SiteId> {
        self.destination_site_id().or(self.leaving_site_id())
    }

    pub fn travel_remaining(&self) -> Option<f32> {
        match self.mode {
            TechnicianMode::EnRoute {
                travel_remaining, ..
            } => Some(travel_remaining),
            _ => None,
        }
    }

    pub fn travel_total(&self) -> Option<f32> {
        match self.mode {
            TechnicianMode::EnRoute { travel_total, .. } => Some(travel_total),
            _ => None,
        }
    }

    pub fn repair_remaining(&self) -> Option<f32> {
        match self.mode {
            TechnicianMode::EnRoute {
                repair_remaining, ..
            }
            | TechnicianMode::WaitingAtSite {
                repair_remaining, ..
            }
            | TechnicianMode::WalkingOnSite {
                repair_remaining, ..
            }
            | TechnicianMode::Repairing {
                repair_remaining, ..
            } => Some(repair_remaining),
            TechnicianMode::Idle | TechnicianMode::LeavingSite { .. } => None,
        }
    }

    pub fn queue_len(&self) -> usize {
        self.dispatch_queue.len()
    }

    pub fn blocks_charger(&self, charger_entity: Entity) -> bool {
        matches!(self.status(), TechStatus::EnRoute | TechStatus::Repairing)
            && self.active_charger() == Some(charger_entity)
    }

    pub fn set_idle(&mut self) {
        self.mode = TechnicianMode::Idle;
        self.job_time_elapsed = 0.0;
    }

    pub fn reset_for_new_day(&mut self) {
        self.current_site_id = None;
        self.job_time_elapsed = 0.0;
        self.mode = TechnicianMode::Idle;
        self.dispatch_queue.clear();
    }

    pub fn begin_en_route(
        &mut self,
        request_id: crate::resources::RepairRequestId,
        charger_entity: Entity,
        site_id: crate::resources::SiteId,
        travel_time: f32,
        repair_remaining: f32,
    ) {
        self.mode = TechnicianMode::EnRoute {
            request_id,
            charger_entity,
            site_id,
            travel_remaining: travel_time,
            travel_total: travel_time,
            repair_remaining,
        };
        self.job_time_elapsed = 0.0;
    }

    pub fn begin_walking_on_site(&mut self) -> bool {
        let (request_id, charger_entity, site_id, repair_remaining) = match self.mode {
            TechnicianMode::EnRoute {
                request_id,
                charger_entity,
                site_id,
                repair_remaining,
                ..
            }
            | TechnicianMode::WaitingAtSite {
                request_id,
                charger_entity,
                site_id,
                repair_remaining,
            } => (request_id, charger_entity, site_id, repair_remaining),
            _ => return false,
        };
        self.mode = TechnicianMode::WalkingOnSite {
            request_id,
            charger_entity,
            site_id,
            repair_remaining,
        };
        true
    }

    pub fn begin_waiting_at_site(&mut self) -> bool {
        let TechnicianMode::EnRoute {
            request_id,
            charger_entity,
            site_id,
            repair_remaining,
            ..
        } = self.mode
        else {
            return false;
        };
        self.mode = TechnicianMode::WaitingAtSite {
            request_id,
            charger_entity,
            site_id,
            repair_remaining,
        };
        true
    }

    pub fn begin_repairing(&mut self) -> bool {
        let TechnicianMode::WalkingOnSite {
            request_id,
            charger_entity,
            site_id,
            repair_remaining,
        } = self.mode
        else {
            return false;
        };
        self.mode = TechnicianMode::Repairing {
            request_id,
            charger_entity,
            site_id,
            repair_remaining,
        };
        true
    }

    pub fn rewind_to_walking_on_site(&mut self) -> bool {
        let TechnicianMode::Repairing {
            request_id,
            charger_entity,
            site_id,
            repair_remaining,
        } = self.mode
        else {
            return false;
        };
        self.mode = TechnicianMode::WalkingOnSite {
            request_id,
            charger_entity,
            site_id,
            repair_remaining,
        };
        true
    }

    pub fn begin_leaving_site(&mut self, site_id: crate::resources::SiteId) {
        self.mode = TechnicianMode::LeavingSite { site_id };
    }

    pub fn tick_travel(&mut self, delta: f32) -> Option<f32> {
        let TechnicianMode::EnRoute {
            ref mut travel_remaining,
            ..
        } = self.mode
        else {
            return None;
        };
        *travel_remaining -= delta;
        if *travel_remaining < 0.0 {
            *travel_remaining = 0.0;
        }
        Some(*travel_remaining)
    }

    pub fn tick_repair(&mut self, delta: f32) -> Option<f32> {
        let repair_remaining = match &mut self.mode {
            TechnicianMode::Repairing {
                repair_remaining, ..
            } => repair_remaining,
            _ => return None,
        };
        *repair_remaining -= delta;
        Some(*repair_remaining)
    }

    pub fn clear_active_request(&mut self) {
        self.mode = TechnicianMode::Idle;
    }

    pub fn complete_job_at(&mut self, site_id: crate::resources::SiteId) {
        self.current_site_id = Some(site_id);
    }

    pub fn clear_current_site_if_matches(&mut self, site_id: crate::resources::SiteId) {
        if self.current_site_id == Some(site_id) {
            self.current_site_id = None;
        }
    }

    pub fn set_same_site_job(
        &mut self,
        request_id: crate::resources::RepairRequestId,
        charger_entity: Entity,
        site_id: crate::resources::SiteId,
        repair_remaining: f32,
    ) {
        self.mode = TechnicianMode::WalkingOnSite {
            request_id,
            charger_entity,
            site_id,
            repair_remaining,
        };
        self.job_time_elapsed = 0.0;
    }

    pub fn set_waiting_at_site_job(
        &mut self,
        request_id: crate::resources::RepairRequestId,
        charger_entity: Entity,
        site_id: crate::resources::SiteId,
        repair_remaining: f32,
    ) {
        self.mode = TechnicianMode::WaitingAtSite {
            request_id,
            charger_entity,
            site_id,
            repair_remaining,
        };
        self.job_time_elapsed = 0.0;
    }

    /// Check if a charger is already queued or being serviced
    pub fn is_charger_queued(&self, charger_entity: Entity) -> bool {
        // Check if it's the current target
        if self.active_charger() == Some(charger_entity) {
            return true;
        }
        // Check if it's in the queue
        self.dispatch_queue
            .iter()
            .any(|q| q.charger_entity == charger_entity)
    }

    /// Queue a dispatch request (skips duplicates)
    /// Returns true if added, false if already queued
    pub fn queue_dispatch(
        &mut self,
        request_id: crate::resources::RepairRequestId,
        charger_entity: Entity,
        charger_id: String,
        site_id: crate::resources::SiteId,
    ) -> bool {
        if self.is_charger_queued(charger_entity) {
            return false;
        }
        self.dispatch_queue.push_back(QueuedDispatch {
            request_id,
            charger_entity,
            charger_id,
            site_id,
        });
        true
    }

    pub fn queue_dispatch_front(
        &mut self,
        request_id: crate::resources::RepairRequestId,
        charger_entity: Entity,
        charger_id: String,
        site_id: crate::resources::SiteId,
    ) -> bool {
        if self.is_charger_queued(charger_entity) {
            return false;
        }
        self.dispatch_queue.push_front(QueuedDispatch {
            request_id,
            charger_entity,
            charger_id,
            site_id,
        });
        true
    }

    /// Find the index of the first queued dispatch targeting the given site.
    ///
    /// Used to chain jobs at the same location without the technician walking
    /// to the exit and re-entering.  The caller is responsible for validating
    /// that the found charger still has a fault requiring a technician.
    pub fn find_same_site_dispatch_index(
        &self,
        site_id: crate::resources::SiteId,
    ) -> Option<usize> {
        self.dispatch_queue
            .iter()
            .position(|queued| queued.site_id == site_id)
    }

    pub fn has_queued_dispatch_for_site(&self, site_id: crate::resources::SiteId) -> bool {
        self.dispatch_queue
            .iter()
            .any(|queued| queued.site_id == site_id)
    }

    /// Pop the next dispatch, preferring the currently viewed site when possible.
    pub fn pop_next_dispatch_for_site(
        &mut self,
        preferred_site_id: Option<crate::resources::SiteId>,
    ) -> Option<QueuedDispatch> {
        if let Some(site_id) = preferred_site_id
            && let Some(idx) = self
                .dispatch_queue
                .iter()
                .position(|queued| queued.site_id == site_id)
        {
            return self.dispatch_queue.remove(idx);
        }

        self.dispatch_queue.pop_front()
    }

    /// Pop the next dispatch from the queue
    pub fn pop_next_dispatch(&mut self) -> Option<QueuedDispatch> {
        self.dispatch_queue.pop_front()
    }

    /// Get ETA string for display
    pub fn eta_string(&self) -> String {
        match self.status() {
            TechStatus::Idle => "Available".to_string(),
            TechStatus::EnRoute => {
                let mins = (self.travel_remaining().unwrap_or(0.0) / 60.0).ceil() as i32;
                format!("Arriving in {mins}m")
            }
            TechStatus::WaitingAtSite => "Waiting for visible site".to_string(),
            TechStatus::WalkingOnSite => "Walking to charger".to_string(),
            TechStatus::Repairing => {
                let mins = (self.repair_remaining().unwrap_or(0.0) / 60.0).ceil() as i32;
                format!("Repairing ({mins}m left)")
            }
            TechStatus::LeavingSite => "Leaving site".to_string(),
        }
    }

    /// Calculate total cost for current job (travel + repair time * hourly rate)
    pub fn calculate_job_cost(&self) -> f32 {
        let hours = self.job_time_elapsed / 3600.0;
        hours * TECHNICIAN_HOURLY_RATE
    }

    /// Get travel progress (0.0 to 1.0)
    pub fn travel_progress(&self) -> f32 {
        let Some(travel_total) = self.travel_total() else {
            return 1.0;
        };
        if travel_total <= 0.0 {
            return 1.0;
        }
        let travel_remaining = self.travel_remaining().unwrap_or(0.0);
        1.0 - (travel_remaining / travel_total).clamp(0.0, 1.0)
    }

    /// Get current location display name
    pub fn current_location_name(&self, multi_site: &crate::resources::MultiSiteManager) -> String {
        match self.status() {
            TechStatus::Idle => {
                if let Some(site_id) = self.current_site_id {
                    if let Some(site) = multi_site.get_site(site_id) {
                        format!("At {}", site.name)
                    } else {
                        "Available".to_string()
                    }
                } else {
                    "Available (No Site)".to_string()
                }
            }
            TechStatus::EnRoute => {
                if let Some(site_id) = self.destination_site_id() {
                    if let Some(site) = multi_site.get_site(site_id) {
                        format!("Traveling to {}", site.name)
                    } else {
                        "Traveling".to_string()
                    }
                } else {
                    "Traveling".to_string()
                }
            }
            TechStatus::WaitingAtSite => {
                if let Some(site_id) = self.destination_site_id() {
                    if let Some(site) = multi_site.get_site(site_id) {
                        format!("Waiting at {} to continue", site.name)
                    } else {
                        "Waiting for visible site".to_string()
                    }
                } else {
                    "Waiting for visible site".to_string()
                }
            }
            TechStatus::WalkingOnSite => {
                if let Some(site_id) = self.destination_site_id() {
                    if let Some(site) = multi_site.get_site(site_id) {
                        format!("At {} (walking)", site.name)
                    } else {
                        "Walking to charger".to_string()
                    }
                } else {
                    "Walking to charger".to_string()
                }
            }
            TechStatus::Repairing => {
                if let Some(site_id) = self.destination_site_id() {
                    if let Some(site) = multi_site.get_site(site_id) {
                        format!("Repairing at {}", site.name)
                    } else {
                        "Repairing".to_string()
                    }
                } else {
                    "Repairing".to_string()
                }
            }
            TechStatus::LeavingSite => {
                if let Some(site_id) = self.leaving_site_id().or(self.current_site_id) {
                    if let Some(site) = multi_site.get_site(site_id) {
                        format!("Leaving {}", site.name)
                    } else {
                        "Leaving site".to_string()
                    }
                } else {
                    "Leaving site".to_string()
                }
            }
        }
    }
}

/// Calculate travel time between two sites based on their archetypes
/// Returns travel time in game seconds
pub fn calculate_travel_time(
    from: crate::resources::SiteArchetype,
    to: crate::resources::SiteArchetype,
) -> f32 {
    // Base times in minutes (convert to seconds)
    const BASE_TRAVEL: f32 = 10.0 * 60.0; // 10 min within same area
    const MEDIUM_TRAVEL: f32 = 20.0 * 60.0; // 20 min cross-district
    const LONG_TRAVEL: f32 = 35.0 * 60.0; // 35 min across town

    // Same archetype = quick (5 min) - sites in same area
    if from == to {
        return 5.0 * 60.0;
    }

    use crate::resources::SiteArchetype;

    match (from, to) {
        // Commercial cluster (short distances)
        (SiteArchetype::ParkingLot, SiteArchetype::GasStation)
        | (SiteArchetype::GasStation, SiteArchetype::ParkingLot) => BASE_TRAVEL,

        // Fleet sites are far from everything
        (SiteArchetype::FleetDepot, _) | (_, SiteArchetype::FleetDepot) => LONG_TRAVEL,

        // Default: medium travel
        _ => MEDIUM_TRAVEL,
    }
}

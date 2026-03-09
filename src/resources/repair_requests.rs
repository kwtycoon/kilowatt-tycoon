//! Durable repair request tracking for technician-required charger faults.

use std::collections::HashMap;

use bevy::prelude::*;

use crate::components::charger::FaultType;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RepairRequestId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RepairRequestSource {
    FaultOccurrence,
    DriverDiscovery,
    OemDetection,
    ManualDispatch,
    AutoDispatch,
    Retry,
    Reconciliation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RepairResolution {
    Resolved,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RepairRequestStatus {
    OpenUndiscovered,
    OpenDiscovered,
    Queued,
    EnRoute,
    WaitingAtSite,
    WalkingOnSite,
    Repairing,
    NeedsRetry,
    Resolved,
    Cancelled,
}

impl RepairRequestStatus {
    pub fn is_open(self) -> bool {
        !matches!(self, Self::Resolved | Self::Cancelled)
    }
}

#[derive(Debug, Clone)]
pub struct RepairRequest {
    pub id: RepairRequestId,
    pub charger_entity: Entity,
    pub charger_id: String,
    pub site_id: crate::resources::SiteId,
    pub fault_type: FaultType,
    pub occurred_at: f32,
    pub discovered_at: Option<f32>,
    pub status: RepairRequestStatus,
    pub attempt_count: u32,
    pub last_dispatch_at: Option<f32>,
    pub resolved_at: Option<f32>,
    pub resolution: Option<RepairResolution>,
    pub source: RepairRequestSource,
    pub dispatch_costs_recorded: bool,
}

#[derive(Resource, Debug, Default, Clone)]
pub struct RepairRequestRegistry {
    next_id: u64,
    requests: HashMap<RepairRequestId, RepairRequest>,
    active_by_charger: HashMap<Entity, RepairRequestId>,
}

impl RepairRequestRegistry {
    pub fn get(&self, id: RepairRequestId) -> Option<&RepairRequest> {
        self.requests.get(&id)
    }

    pub fn get_mut(&mut self, id: RepairRequestId) -> Option<&mut RepairRequest> {
        self.requests.get_mut(&id)
    }

    pub fn active_for_charger(&self, charger_entity: Entity) -> Option<&RepairRequest> {
        let request_id = self.active_by_charger.get(&charger_entity)?;
        self.requests.get(request_id)
    }

    pub fn active_request_id_for_charger(&self, charger_entity: Entity) -> Option<RepairRequestId> {
        self.active_by_charger.get(&charger_entity).copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = &RepairRequest> {
        self.requests.values()
    }

    pub fn create_or_update_for_fault(
        &mut self,
        charger_entity: Entity,
        charger_id: String,
        site_id: crate::resources::SiteId,
        fault_type: FaultType,
        occurred_at: f32,
        source: RepairRequestSource,
    ) -> RepairRequestId {
        if let Some(request_id) = self.active_by_charger.get(&charger_entity).copied()
            && let Some(request) = self.requests.get_mut(&request_id)
        {
            request.charger_id = charger_id;
            request.site_id = site_id;
            request.fault_type = fault_type;
            request.occurred_at = occurred_at;
            request.source = source;
            return request_id;
        }

        let request_id = RepairRequestId(self.next_id);
        self.next_id += 1;
        let request = RepairRequest {
            id: request_id,
            charger_entity,
            charger_id,
            site_id,
            fault_type,
            occurred_at,
            discovered_at: None,
            status: RepairRequestStatus::OpenUndiscovered,
            attempt_count: 0,
            last_dispatch_at: None,
            resolved_at: None,
            resolution: None,
            source,
            dispatch_costs_recorded: false,
        };
        self.requests.insert(request_id, request);
        self.active_by_charger.insert(charger_entity, request_id);
        request_id
    }

    pub fn mark_discovered(
        &mut self,
        charger_entity: Entity,
        discovered_at: f32,
        source: RepairRequestSource,
    ) -> Option<RepairRequestId> {
        let request_id = self.active_by_charger.get(&charger_entity).copied()?;
        let request = self.requests.get_mut(&request_id)?;
        if request.discovered_at.is_none() {
            request.discovered_at = Some(discovered_at);
        }
        if matches!(
            request.status,
            RepairRequestStatus::OpenUndiscovered | RepairRequestStatus::NeedsRetry
        ) {
            request.status = RepairRequestStatus::OpenDiscovered;
        }
        request.source = source;
        Some(request_id)
    }

    pub fn queue(
        &mut self,
        request_id: RepairRequestId,
        dispatched_at: f32,
        source: RepairRequestSource,
    ) -> bool {
        let Some(request) = self.requests.get_mut(&request_id) else {
            return false;
        };
        if !request.status.is_open() {
            return false;
        }
        if matches!(
            request.status,
            RepairRequestStatus::Queued
                | RepairRequestStatus::EnRoute
                | RepairRequestStatus::WaitingAtSite
                | RepairRequestStatus::WalkingOnSite
                | RepairRequestStatus::Repairing
        ) {
            return false;
        }
        request.status = RepairRequestStatus::Queued;
        request.last_dispatch_at = Some(dispatched_at);
        request.source = source;
        true
    }

    pub fn set_status(&mut self, request_id: RepairRequestId, status: RepairRequestStatus) -> bool {
        let Some(request) = self.requests.get_mut(&request_id) else {
            return false;
        };
        if !request.status.is_open()
            && !matches!(
                status,
                RepairRequestStatus::Resolved | RepairRequestStatus::Cancelled
            )
        {
            return false;
        }
        request.status = status;
        true
    }

    pub fn mark_retry_needed(
        &mut self,
        request_id: RepairRequestId,
        dispatched_at: Option<f32>,
    ) -> bool {
        let Some(request) = self.requests.get_mut(&request_id) else {
            return false;
        };
        if !request.status.is_open() {
            return false;
        }
        request.attempt_count += 1;
        request.status = RepairRequestStatus::NeedsRetry;
        if let Some(at) = dispatched_at {
            request.last_dispatch_at = Some(at);
        }
        true
    }

    pub fn dispatch_costs_recorded(&self, request_id: RepairRequestId) -> bool {
        self.requests
            .get(&request_id)
            .map(|request| request.dispatch_costs_recorded)
            .unwrap_or(false)
    }

    pub fn mark_dispatch_costs_recorded(&mut self, request_id: RepairRequestId) -> bool {
        let Some(request) = self.requests.get_mut(&request_id) else {
            return false;
        };
        let was_already_recorded = request.dispatch_costs_recorded;
        request.dispatch_costs_recorded = true;
        !was_already_recorded
    }

    /// Normalize in-flight technician work to a re-dispatchable state at day end.
    ///
    /// We preserve the durable request and its billing history, but clear runtime-only
    /// execution states so the next day can rebuild technician work from a clean slate.
    pub fn reset_for_new_day(&mut self) {
        for request in self.requests.values_mut() {
            if !request.status.is_open() {
                continue;
            }

            request.status = match request.status {
                RepairRequestStatus::Queued
                | RepairRequestStatus::EnRoute
                | RepairRequestStatus::WaitingAtSite
                | RepairRequestStatus::WalkingOnSite
                | RepairRequestStatus::Repairing => RepairRequestStatus::OpenDiscovered,
                status => status,
            };

            if request.discovered_at.is_none() {
                request.discovered_at = request.last_dispatch_at.or(Some(request.occurred_at));
            }
        }
    }

    pub fn resolve(
        &mut self,
        request_id: RepairRequestId,
        resolved_at: f32,
        resolution: RepairResolution,
    ) -> bool {
        let Some(request) = self.requests.get_mut(&request_id) else {
            return false;
        };
        request.resolved_at = Some(resolved_at);
        request.resolution = Some(resolution);
        request.status = match resolution {
            RepairResolution::Resolved => RepairRequestStatus::Resolved,
            RepairResolution::Cancelled => RepairRequestStatus::Cancelled,
        };
        self.active_by_charger.remove(&request.charger_entity);
        true
    }

    pub fn resolve_for_charger(
        &mut self,
        charger_entity: Entity,
        resolved_at: f32,
        resolution: RepairResolution,
    ) -> bool {
        let Some(request_id) = self.active_by_charger.get(&charger_entity).copied() else {
            return false;
        };
        self.resolve(request_id, resolved_at, resolution)
    }
}

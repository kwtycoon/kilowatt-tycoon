//! Technician resource for field repairs

use bevy::prelude::*;

/// Technician hourly rate for OpEx calculations
pub const TECHNICIAN_HOURLY_RATE: f32 = 150.0; // $/hour

/// Base travel time to reach site (game seconds)
pub const BASE_TRAVEL_TIME: f32 = 1800.0; // 30 minutes

/// Queued dispatch request for technician
#[derive(Debug, Clone)]
pub struct QueuedDispatch {
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
    /// Technician entity spawned, walking to charger on-site
    WalkingOnSite,
    /// Technician is performing repair on-site
    Repairing,
    /// Technician finished repair and is walking to exit
    LeavingSite,
}

/// Technician state - single shared technician
#[derive(Resource, Debug, Clone)]
pub struct TechnicianState {
    /// Current status
    pub status: TechStatus,
    /// Target charger being repaired (if any)
    pub target_charger: Option<Entity>,
    /// Current site location (None = not at any site yet)
    pub current_site_id: Option<crate::resources::SiteId>,
    /// Destination site for current job
    pub destination_site_id: Option<crate::resources::SiteId>,
    /// Remaining travel time in game seconds
    pub travel_remaining: f32,
    /// Total travel time for current journey (for progress calculation)
    pub travel_total: f32,
    /// Remaining repair time in game seconds
    pub repair_remaining: f32,
    /// Total time spent on this job (for billing)
    pub job_time_elapsed: f32,
    /// Queue of pending dispatch requests
    pub dispatch_queue: Vec<QueuedDispatch>,
}

impl Default for TechnicianState {
    fn default() -> Self {
        Self {
            status: TechStatus::Idle,
            target_charger: None,
            current_site_id: None,
            destination_site_id: None,
            travel_remaining: 0.0,
            travel_total: 0.0,
            repair_remaining: 0.0,
            job_time_elapsed: 0.0,
            dispatch_queue: Vec::new(),
        }
    }
}

impl TechnicianState {
    /// Check if technician is available for dispatch
    pub fn is_available(&self) -> bool {
        self.status == TechStatus::Idle
    }

    /// Check if a charger is already queued or being serviced
    pub fn is_charger_queued(&self, charger_entity: Entity) -> bool {
        // Check if it's the current target
        if self.target_charger == Some(charger_entity) {
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
        charger_entity: Entity,
        charger_id: String,
        site_id: crate::resources::SiteId,
    ) -> bool {
        if self.is_charger_queued(charger_entity) {
            return false;
        }
        self.dispatch_queue.push(QueuedDispatch {
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

    /// Pop the next dispatch from the queue
    pub fn pop_next_dispatch(&mut self) -> Option<QueuedDispatch> {
        if self.dispatch_queue.is_empty() {
            None
        } else {
            Some(self.dispatch_queue.remove(0))
        }
    }

    /// Get ETA string for display
    pub fn eta_string(&self) -> String {
        match self.status {
            TechStatus::Idle => "Available".to_string(),
            TechStatus::EnRoute => {
                let mins = (self.travel_remaining / 60.0).ceil() as i32;
                format!("Arriving in {mins}m")
            }
            TechStatus::WalkingOnSite => "Walking to charger".to_string(),
            TechStatus::Repairing => {
                let mins = (self.repair_remaining / 60.0).ceil() as i32;
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
        if self.travel_total <= 0.0 {
            return 1.0;
        }
        1.0 - (self.travel_remaining / self.travel_total).clamp(0.0, 1.0)
    }

    /// Get current location display name
    pub fn current_location_name(&self, multi_site: &crate::resources::MultiSiteManager) -> String {
        match self.status {
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
                if let Some(site_id) = self.destination_site_id {
                    if let Some(site) = multi_site.get_site(site_id) {
                        format!("Traveling to {}", site.name)
                    } else {
                        "Traveling".to_string()
                    }
                } else {
                    "Traveling".to_string()
                }
            }
            TechStatus::WalkingOnSite => {
                if let Some(site_id) = self.destination_site_id {
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
                if let Some(site_id) = self.destination_site_id {
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
                if let Some(site_id) = self.current_site_id {
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

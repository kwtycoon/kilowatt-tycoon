//! Charger queue management for drivers waiting for available chargers

use bevy::prelude::*;
use std::collections::VecDeque;

/// Resource tracking drivers waiting in queue for chargers
#[derive(Resource, Debug, Clone, Default)]
pub struct ChargerQueue {
    /// Drivers waiting for any DCFC charger, ordered by arrival time
    pub dcfc_queue: VecDeque<Entity>,
    /// Drivers waiting for any L2 charger, ordered by arrival time
    pub l2_queue: VecDeque<Entity>,
}

impl ChargerQueue {
    /// Add a driver to the DCFC queue
    pub fn join_dcfc_queue(&mut self, driver_entity: Entity) {
        if !self.dcfc_queue.contains(&driver_entity) {
            self.dcfc_queue.push_back(driver_entity);
        }
    }

    /// Add a driver to the L2 queue
    pub fn join_l2_queue(&mut self, driver_entity: Entity) {
        if !self.l2_queue.contains(&driver_entity) {
            self.l2_queue.push_back(driver_entity);
        }
    }

    /// Remove a driver from the DCFC queue (called when they get a charger or leave)
    pub fn leave_dcfc_queue(&mut self, driver_entity: Entity) {
        self.dcfc_queue.retain(|&e| e != driver_entity);
    }

    /// Remove a driver from the L2 queue
    pub fn leave_l2_queue(&mut self, driver_entity: Entity) {
        self.l2_queue.retain(|&e| e != driver_entity);
    }

    /// Remove a driver from any queue they might be in
    pub fn leave_all_queues(&mut self, driver_entity: Entity) {
        self.leave_dcfc_queue(driver_entity);
        self.leave_l2_queue(driver_entity);
    }

    /// Get the next driver in the DCFC queue without removing them
    pub fn peek_dcfc(&self) -> Option<Entity> {
        self.dcfc_queue.front().copied()
    }

    /// Get the next driver in the L2 queue without removing them
    pub fn peek_l2(&self) -> Option<Entity> {
        self.l2_queue.front().copied()
    }

    /// Pop the next driver from the DCFC queue
    pub fn pop_dcfc(&mut self) -> Option<Entity> {
        self.dcfc_queue.pop_front()
    }

    /// Pop the next driver from the L2 queue
    pub fn pop_l2(&mut self) -> Option<Entity> {
        self.l2_queue.pop_front()
    }

    /// Get position of a driver in the DCFC queue (1-indexed, None if not in queue)
    pub fn dcfc_position(&self, driver_entity: Entity) -> Option<usize> {
        self.dcfc_queue
            .iter()
            .position(|&e| e == driver_entity)
            .map(|p| p + 1)
    }

    /// Get position of a driver in the L2 queue (1-indexed, None if not in queue)
    pub fn l2_position(&self, driver_entity: Entity) -> Option<usize> {
        self.l2_queue
            .iter()
            .position(|&e| e == driver_entity)
            .map(|p| p + 1)
    }

    /// Get total number of drivers waiting
    pub fn total_waiting(&self) -> usize {
        self.dcfc_queue.len() + self.l2_queue.len()
    }

    /// Get DCFC queue length
    pub fn dcfc_queue_len(&self) -> usize {
        self.dcfc_queue.len()
    }

    /// Get L2 queue length
    pub fn l2_queue_len(&self) -> usize {
        self.l2_queue.len()
    }

    /// Clear all queues (called when starting a new day)
    pub fn clear(&mut self) {
        self.dcfc_queue.clear();
        self.l2_queue.clear();
    }
}

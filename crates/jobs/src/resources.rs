//! In-process scheduling only. The host must persist unfinished jobs, verify
//! engine identity and settle external work before acknowledging a quarantine.
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, Weak},
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Resource {
    /// Host-canonical engine binding, not an untrusted URL or a per-job ID.
    Engine(String),
    Gpu,
    VideoOutput,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceError {
    InvalidRequest,
    Busy,
    Quarantined,
    Poisoned,
    Limit,
    WrongRegistry,
    Stale,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Reserved,
    Running,
    Quarantined,
}
struct Record {
    owner: String,
    resources: Vec<Resource>,
    phase: Phase,
}
#[derive(Default)]
struct State {
    sequence: u64,
    capacities: HashMap<Resource, usize>,
    records: HashMap<u64, Record>,
}
#[derive(Clone, Default)]
pub struct Resources {
    state: Arc<Mutex<State>>,
}
/// Non-cloneable execution ownership. A dropped running lease quarantines its
/// entire resource set; it never silently authorizes another external execution.
pub struct ResourceLease {
    state: Arc<Mutex<State>>,
    id: u64,
    released: bool,
}
/// Only produced for quarantined work in this specific coordinator instance.
#[derive(Clone)]
pub struct Settlement {
    state: Weak<Mutex<State>>,
    id: u64,
    owner: String,
    resources: Vec<Resource>,
}
impl Settlement {
    pub fn owner(&self) -> &str {
        &self.owner
    }
    pub fn resources(&self) -> &[Resource] {
        &self.resources
    }
}
impl Resources {
    /// Default capacity is one per engine, one GPU and one video-output worker.
    /// Overrides are fixed before work starts. Quarantine blocks all capacity.
    pub fn with_capacities(capacities: Vec<(Resource, usize)>) -> Result<Self, ResourceError> {
        let mut state = State::default();
        if capacities.len() > 1024 {
            return Err(ResourceError::Limit);
        }
        for (key, limit) in capacities {
            validate_resource(&key)?;
            if !(1..=64).contains(&limit) || state.capacities.insert(key, limit).is_some() {
                return Err(ResourceError::InvalidRequest);
            }
        }
        Ok(Self {
            state: Arc::new(Mutex::new(state)),
        })
    }
    /// Rebuild from persisted unfinished work BEFORE accepting new jobs.
    /// Restoring quarantine does not restart or terminate external processes.
    pub fn recover(
        capacities: Vec<(Resource, usize)>,
        unfinished: Vec<(String, Vec<Resource>)>,
    ) -> Result<Self, ResourceError> {
        if unfinished.len() > 1024 {
            return Err(ResourceError::Limit);
        }
        let registry = Self::with_capacities(capacities)?;
        {
            let mut state = registry.state.lock().map_err(|_| ResourceError::Poisoned)?;
            let mut owners = HashSet::new();
            for (owner, resources) in unfinished {
                validate_request(&owner, &resources)?;
                if !owners.insert(owner.clone()) {
                    return Err(ResourceError::InvalidRequest);
                }
                state.sequence += 1;
                let id = state.sequence;
                state.records.insert(
                    id,
                    Record {
                        owner,
                        resources,
                        phase: Phase::Quarantined,
                    },
                );
            }
        }
        Ok(registry)
    }
    /// All resources are reserved atomically or none are. No waiting queue and
    /// no partially held GPU slot while waiting on an engine.
    pub fn try_acquire(
        &self,
        owner: &str,
        resources: Vec<Resource>,
    ) -> Result<ResourceLease, ResourceError> {
        validate_request(owner, &resources)?;
        let mut state = self.state.lock().map_err(|_| ResourceError::Poisoned)?;
        if state.records.len() >= 1024 {
            return Err(ResourceError::Limit);
        }
        if state.records.values().any(|r| r.owner == owner) {
            return Err(ResourceError::Busy);
        }
        for resource in &resources {
            let users: Vec<_> = state
                .records
                .values()
                .filter(|r| r.resources.contains(resource))
                .collect();
            if users.iter().any(|r| r.phase == Phase::Quarantined) {
                return Err(ResourceError::Quarantined);
            }
            if users.len() >= *state.capacities.get(resource).unwrap_or(&1) {
                return Err(ResourceError::Busy);
            }
        }
        let id = state.sequence.checked_add(1).ok_or(ResourceError::Limit)?;
        state.sequence = id;
        state.records.insert(
            id,
            Record {
                owner: owner.into(),
                resources,
                phase: Phase::Reserved,
            },
        );
        Ok(ResourceLease {
            state: self.state.clone(),
            id,
            released: false,
        })
    }
    /// Includes reserved, running and quarantined leases; read-only evidence.
    pub fn has_owner(&self, owner: &str) -> Result<bool, ResourceError> {
        Ok(self
            .state
            .lock()
            .map_err(|_| ResourceError::Poisoned)?
            .records
            .values()
            .any(|r| r.owner == owner))
    }
    pub fn quarantined(&self) -> Result<Vec<Settlement>, ResourceError> {
        let state = self.state.lock().map_err(|_| ResourceError::Poisoned)?;
        let mut records: Vec<_> = state
            .records
            .iter()
            .filter(|(_, r)| r.phase == Phase::Quarantined)
            .collect();
        records.sort_by_key(|(id, _)| **id);
        Ok(records
            .into_iter()
            .map(|(id, r)| Settlement {
                state: Arc::downgrade(&self.state),
                id: *id,
                owner: r.owner.clone(),
                resources: r.resources.clone(),
            })
            .collect())
    }
    /// Caller must prove external work ended (owned process stopped, or an
    /// external service explicitly settled). A healthy /version is insufficient.
    /// This method performs no process termination and no remote cancellation.
    pub fn confirm_settled(&self, settlement: &Settlement) -> Result<(), ResourceError> {
        if !Weak::ptr_eq(&settlement.state, &Arc::downgrade(&self.state)) {
            return Err(ResourceError::WrongRegistry);
        }
        let mut state = self.state.lock().map_err(|_| ResourceError::Poisoned)?;
        if !state
            .records
            .get(&settlement.id)
            .is_some_and(|r| r.phase == Phase::Quarantined)
        {
            return Err(ResourceError::Stale);
        }
        state.records.remove(&settlement.id);
        Ok(())
    }
}
impl ResourceLease {
    /// Must be called immediately BEFORE dispatching any external work.
    pub fn start(&mut self) -> Result<(), ResourceError> {
        let mut state = self.state.lock().map_err(|_| ResourceError::Poisoned)?;
        let record = state
            .records
            .get_mut(&self.id)
            .ok_or(ResourceError::Stale)?;
        if record.phase != Phase::Reserved {
            return Err(ResourceError::InvalidRequest);
        }
        record.phase = Phase::Running;
        Ok(())
    }
    /// Normal completion or cancellation after the in-flight operation has
    /// actually ended. HTTP deadline/connection failure does not meet this rule.
    pub fn confirm_finished(mut self) -> Result<(), ResourceError> {
        let mut state = self.state.lock().map_err(|_| ResourceError::Poisoned)?;
        if !state
            .records
            .get(&self.id)
            .is_some_and(|r| r.phase == Phase::Running)
        {
            return Err(ResourceError::InvalidRequest);
        }
        state.records.remove(&self.id);
        self.released = true;
        Ok(())
    }
}
impl Drop for ResourceLease {
    fn drop(&mut self) {
        if self.released {
            return;
        }
        // Poison remains fail-closed for future acquisition; recover only to
        // retain the unfinished ownership record, never to bypass poison.
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let Some(record) = state.records.get_mut(&self.id) else {
            return;
        };
        if record.phase == Phase::Reserved {
            state.records.remove(&self.id);
        } else {
            record.phase = Phase::Quarantined;
        }
    }
}
fn validate_label(value: &str) -> Result<(), ResourceError> {
    if value.trim().is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
        return Err(ResourceError::InvalidRequest);
    }
    Ok(())
}
fn validate_resource(value: &Resource) -> Result<(), ResourceError> {
    if let Resource::Engine(key) = value {
        validate_label(key)?;
    }
    Ok(())
}

fn validate_request(owner: &str, resources: &[Resource]) -> Result<(), ResourceError> {
    validate_label(owner)?;
    if resources.is_empty() || resources.len() > 8 {
        return Err(ResourceError::InvalidRequest);
    }
    let mut seen = HashSet::new();
    for resource in resources {
        validate_resource(resource)?;
        if !seen.insert(resource) {
            return Err(ResourceError::InvalidRequest);
        }
    }
    Ok(())
}

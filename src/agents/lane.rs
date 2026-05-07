//! Lane orchestration & event system [10.8]
//!
//! Parallel execution lanes with event-driven status updates,
//! branch collision detection, and commit provenance tracking.

use crate::error::{FuseError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique identifier for a lane.
pub type LaneId = String;

/// State of a lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LaneState {
    Pending,
    Running,
    Blocked,
    Completed,
    Failed,
}

/// An event emitted by a lane.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaneEvent {
    pub lane_id: LaneId,
    pub event_type: LaneEventType,
    pub timestamp: DateTime<Utc>,
    pub message: Option<String>,
}

/// Types of lane events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LaneEventType {
    Started,
    Blocked { reason: String },
    Unblocked,
    CommitCreated { sha: String, branch: String },
    TestsPassed,
    TestsFailed { details: String },
    Completed,
    Failed { error: String },
}

/// Commit provenance — tracks where a commit came from.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitProvenance {
    pub sha: String,
    pub branch: String,
    pub lane_id: LaneId,
    pub task_id: Option<String>,
    pub timestamp: DateTime<Utc>,
}

/// A single execution lane.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lane {
    pub id: LaneId,
    pub task_id: String,
    pub branch: String,
    pub state: LaneState,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub commits: Vec<CommitProvenance>,
    pub events: Vec<LaneEvent>,
}

impl Lane {
    pub fn new(
        id: impl Into<String>,
        task_id: impl Into<String>,
        branch: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: id.into(),
            task_id: task_id.into(),
            branch: branch.into(),
            state: LaneState::Pending,
            created_at: now,
            updated_at: now,
            commits: Vec::new(),
            events: Vec::new(),
        }
    }

    /// Emit an event and update state accordingly.
    fn emit(&mut self, event_type: LaneEventType, message: Option<String>) {
        let event = LaneEvent {
            lane_id: self.id.clone(),
            event_type: event_type.clone(),
            timestamp: Utc::now(),
            message,
        };
        self.events.push(event);
        self.updated_at = Utc::now();

        // Update state based on event
        match event_type {
            LaneEventType::Started => self.state = LaneState::Running,
            LaneEventType::Blocked { .. } => self.state = LaneState::Blocked,
            LaneEventType::Unblocked => self.state = LaneState::Running,
            LaneEventType::Completed => self.state = LaneState::Completed,
            LaneEventType::Failed { .. } => self.state = LaneState::Failed,
            _ => {}
        }
    }

    pub fn start(&mut self) {
        self.emit(LaneEventType::Started, None);
    }

    pub fn block(&mut self, reason: impl Into<String>) {
        let reason = reason.into();
        self.emit(
            LaneEventType::Blocked {
                reason: reason.clone(),
            },
            Some(reason),
        );
    }

    pub fn unblock(&mut self) {
        self.emit(LaneEventType::Unblocked, None);
    }

    pub fn record_commit(&mut self, sha: impl Into<String>) {
        let sha = sha.into();
        let provenance = CommitProvenance {
            sha: sha.clone(),
            branch: self.branch.clone(),
            lane_id: self.id.clone(),
            task_id: Some(self.task_id.clone()),
            timestamp: Utc::now(),
        };
        self.commits.push(provenance);
        self.emit(
            LaneEventType::CommitCreated {
                sha,
                branch: self.branch.clone(),
            },
            None,
        );
    }

    pub fn complete(&mut self) {
        self.emit(LaneEventType::Completed, None);
    }

    pub fn fail(&mut self, error: impl Into<String>) {
        let error = error.into();
        self.emit(
            LaneEventType::Failed {
                error: error.clone(),
            },
            Some(error),
        );
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self.state, LaneState::Completed | LaneState::Failed)
    }
}

/// Manages multiple parallel lanes, detects collisions.
pub struct LaneBoard {
    lanes: HashMap<LaneId, Lane>,
}

impl LaneBoard {
    pub fn new() -> Self {
        Self {
            lanes: HashMap::new(),
        }
    }

    /// Create a new lane.
    pub fn create_lane(
        &mut self,
        id: impl Into<String>,
        task_id: impl Into<String>,
        branch: impl Into<String>,
    ) -> Result<&Lane> {
        let id = id.into();
        if self.lanes.contains_key(&id) {
            return Err(FuseError::AgentError(format!("Lane already exists: {id}")));
        }
        let lane = Lane::new(id.clone(), task_id, branch);
        self.lanes.insert(id.clone(), lane);
        Ok(self.lanes.get(&id).unwrap())
    }

    /// Get a lane by ID.
    pub fn get(&self, id: &str) -> Option<&Lane> {
        self.lanes.get(id)
    }

    /// Get a mutable lane by ID.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut Lane> {
        self.lanes.get_mut(id)
    }

    /// List all active (non-terminal) lanes.
    pub fn active_lanes(&self) -> Vec<&Lane> {
        self.lanes.values().filter(|l| !l.is_terminal()).collect()
    }

    /// Detect branch collisions — multiple active lanes on the same branch.
    pub fn detect_collisions(&self) -> Vec<BranchCollision> {
        let mut branch_lanes: HashMap<&str, Vec<&Lane>> = HashMap::new();

        for lane in self.lanes.values() {
            if !lane.is_terminal() {
                branch_lanes
                    .entry(lane.branch.as_str())
                    .or_default()
                    .push(lane);
            }
        }

        branch_lanes
            .into_iter()
            .filter(|(_, lanes)| lanes.len() > 1)
            .map(|(branch, lanes)| BranchCollision {
                branch: branch.to_string(),
                lane_ids: lanes.iter().map(|l| l.id.clone()).collect(),
            })
            .collect()
    }

    /// Remove completed/failed lanes.
    pub fn cleanup_terminal(&mut self) -> usize {
        let before = self.lanes.len();
        self.lanes.retain(|_, l| !l.is_terminal());
        before - self.lanes.len()
    }

    /// Total lane count.
    pub fn count(&self) -> usize {
        self.lanes.len()
    }

    /// Collect all events across all lanes, sorted by time.
    pub fn all_events(&self) -> Vec<&LaneEvent> {
        let mut events: Vec<&LaneEvent> =
            self.lanes.values().flat_map(|l| l.events.iter()).collect();
        events.sort_by_key(|e| e.timestamp);
        events
    }
}

impl Default for LaneBoard {
    fn default() -> Self {
        Self::new()
    }
}

/// A branch collision — multiple active lanes working on the same branch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchCollision {
    pub branch: String,
    pub lane_ids: Vec<LaneId>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lane_lifecycle() {
        let mut lane = Lane::new("l1", "t1", "feat/thing");
        assert_eq!(lane.state, LaneState::Pending);

        lane.start();
        assert_eq!(lane.state, LaneState::Running);

        lane.record_commit("abc123");
        assert_eq!(lane.commits.len(), 1);
        assert_eq!(lane.commits[0].sha, "abc123");

        lane.complete();
        assert_eq!(lane.state, LaneState::Completed);
        assert!(lane.is_terminal());
    }

    #[test]
    fn test_lane_block_unblock() {
        let mut lane = Lane::new("l1", "t1", "main");
        lane.start();
        lane.block("merge conflict");
        assert_eq!(lane.state, LaneState::Blocked);

        lane.unblock();
        assert_eq!(lane.state, LaneState::Running);
    }

    #[test]
    fn test_lane_fail() {
        let mut lane = Lane::new("l1", "t1", "main");
        lane.start();
        lane.fail("compilation error");
        assert_eq!(lane.state, LaneState::Failed);
        assert!(lane.is_terminal());
    }

    #[test]
    fn test_lane_events_recorded() {
        let mut lane = Lane::new("l1", "t1", "main");
        lane.start();
        lane.record_commit("a1b2c3");
        lane.complete();
        assert_eq!(lane.events.len(), 3);
    }

    #[test]
    fn test_board_create_lane() {
        let mut board = LaneBoard::new();
        board.create_lane("l1", "t1", "feat/a").unwrap();
        assert_eq!(board.count(), 1);
        assert!(board.get("l1").is_some());
    }

    #[test]
    fn test_board_duplicate_lane_error() {
        let mut board = LaneBoard::new();
        board.create_lane("l1", "t1", "feat/a").unwrap();
        assert!(board.create_lane("l1", "t2", "feat/b").is_err());
    }

    #[test]
    fn test_board_active_lanes() {
        let mut board = LaneBoard::new();
        board.create_lane("l1", "t1", "feat/a").unwrap();
        board.create_lane("l2", "t2", "feat/b").unwrap();
        board.get_mut("l1").unwrap().start();
        board.get_mut("l2").unwrap().start();
        board.get_mut("l2").unwrap().complete();

        assert_eq!(board.active_lanes().len(), 1);
    }

    #[test]
    fn test_collision_detection() {
        let mut board = LaneBoard::new();
        board.create_lane("l1", "t1", "main").unwrap();
        board.create_lane("l2", "t2", "main").unwrap();
        board.get_mut("l1").unwrap().start();
        board.get_mut("l2").unwrap().start();

        let collisions = board.detect_collisions();
        assert_eq!(collisions.len(), 1);
        assert_eq!(collisions[0].branch, "main");
        assert_eq!(collisions[0].lane_ids.len(), 2);
    }

    #[test]
    fn test_no_collision_different_branches() {
        let mut board = LaneBoard::new();
        board.create_lane("l1", "t1", "feat/a").unwrap();
        board.create_lane("l2", "t2", "feat/b").unwrap();
        board.get_mut("l1").unwrap().start();
        board.get_mut("l2").unwrap().start();

        assert!(board.detect_collisions().is_empty());
    }

    #[test]
    fn test_no_collision_terminal_lanes() {
        let mut board = LaneBoard::new();
        board.create_lane("l1", "t1", "main").unwrap();
        board.create_lane("l2", "t2", "main").unwrap();
        board.get_mut("l1").unwrap().start();
        board.get_mut("l1").unwrap().complete(); // terminal
        board.get_mut("l2").unwrap().start();

        assert!(board.detect_collisions().is_empty());
    }

    #[test]
    fn test_cleanup_terminal() {
        let mut board = LaneBoard::new();
        board.create_lane("l1", "t1", "feat/a").unwrap();
        board.create_lane("l2", "t2", "feat/b").unwrap();
        board.get_mut("l1").unwrap().start();
        board.get_mut("l1").unwrap().complete();

        let cleaned = board.cleanup_terminal();
        assert_eq!(cleaned, 1);
        assert_eq!(board.count(), 1);
    }

    #[test]
    fn test_all_events_sorted() {
        let mut board = LaneBoard::new();
        board.create_lane("l1", "t1", "a").unwrap();
        board.create_lane("l2", "t2", "b").unwrap();
        board.get_mut("l1").unwrap().start();
        board.get_mut("l2").unwrap().start();
        board.get_mut("l1").unwrap().complete();

        let events = board.all_events();
        assert!(events.len() >= 3);
        for window in events.windows(2) {
            assert!(window[0].timestamp <= window[1].timestamp);
        }
    }

    #[test]
    fn test_commit_provenance() {
        let mut lane = Lane::new("l1", "t1", "feat/x");
        lane.start();
        lane.record_commit("sha1");
        lane.record_commit("sha2");

        assert_eq!(lane.commits.len(), 2);
        assert_eq!(lane.commits[0].lane_id, "l1");
        assert_eq!(lane.commits[0].branch, "feat/x");
        assert_eq!(lane.commits[1].sha, "sha2");
    }

    #[test]
    fn test_lane_serde() {
        let mut lane = Lane::new("l1", "t1", "main");
        lane.start();
        let json = serde_json::to_string(&lane).unwrap();
        let back: Lane = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, "l1");
        assert_eq!(back.state, LaneState::Running);
    }
}

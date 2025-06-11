use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayerColor {
    White,
    Black,
}

impl PlayerColor {
    pub fn as_str(&self) -> &'static str {
        match self {
            PlayerColor::White => "white",
            PlayerColor::Black => "black",
        }
    }
}

impl FromStr for PlayerColor {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "white" => Ok(PlayerColor::White),
            "black" => Ok(PlayerColor::Black),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameStatus {
    Pending,
    Active,
    Completed,
    Abandoned,
}

impl GameStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            GameStatus::Pending => "pending",
            GameStatus::Active => "active",
            GameStatus::Completed => "completed",
            GameStatus::Abandoned => "abandoned",
        }
    }
}

impl FromStr for GameStatus {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(GameStatus::Pending),
            "active" => Ok(GameStatus::Active),
            "completed" => Ok(GameStatus::Completed),
            "abandoned" => Ok(GameStatus::Abandoned),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameResult {
    Win,
    Loss,
    Draw,
    Abandoned,
}

impl GameResult {
    pub fn as_str(&self) -> &'static str {
        match self {
            GameResult::Win => "win",
            GameResult::Loss => "loss",
            GameResult::Draw => "draw",
            GameResult::Abandoned => "abandoned",
        }
    }
}

impl FromStr for GameResult {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "win" => Ok(GameResult::Win),
            "loss" => Ok(GameResult::Loss),
            "draw" => Ok(GameResult::Draw),
            "abandoned" => Ok(GameResult::Abandoned),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    pub id: String,
    pub opponent_peer_id: String,
    pub my_color: PlayerColor,
    pub status: GameStatus,
    pub created_at: i64,
    pub updated_at: i64,
    pub completed_at: Option<i64>,
    pub result: Option<GameResult>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: Option<i64>, // Auto-increment from database
    pub game_id: String,
    pub message_type: String,
    pub content: String, // JSON serialized message content
    pub signature: String,
    pub sender_peer_id: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameMetadata {
    pub initial_fen: Option<String>,
    pub time_control: Option<TimeControl>,
    pub rated: Option<bool>,
    pub tournament_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeControl {
    pub initial_time_ms: u64,
    pub increment_ms: u64,
}

use serde::{Deserialize, Serialize};

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

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "white" => Some(PlayerColor::White),
            "black" => Some(PlayerColor::Black),
            _ => None,
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

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "pending" => Some(GameStatus::Pending),
            "active" => Some(GameStatus::Active),
            "completed" => Some(GameStatus::Completed),
            "abandoned" => Some(GameStatus::Abandoned),
            _ => None,
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

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "win" => Some(GameResult::Win),
            "loss" => Some(GameResult::Loss),
            "draw" => Some(GameResult::Draw),
            "abandoned" => Some(GameResult::Abandoned),
            _ => None,
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

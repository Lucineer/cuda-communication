/*!
# cuda-communication

Inter-agent communication.

Agents don't just act — they talk. Communication is how fleets coordinate,
how knowledge spreads, how conflicts get resolved without violence.

This crate provides:
- Message types (request, inform, command, query, negotiate)
- Intent extraction from messages
- Conversation framing (turns, context, grounding)
- Protocol negotiation (shared vocabulary, turn-taking)
- Communication budget (energy cost per message)
*/

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

/// Message intent — what is this message trying to do?
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Intent {
    Inform,      // sharing information
    Request,     // asking for something
    Command,     // telling to do something
    Query,       // asking a question
    Offer,       // proposing an exchange
    Accept,      // agreeing
    Reject,      // refusing
    Warn,        // alerting to danger
    Apologize,   // acknowledging fault
    Thank,       // expressing gratitude
}

/// Message priority
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum MessagePriority {
    Background = 0,  // gossip, status updates
    Normal = 1,
    Important = 2,   // task coordination
    Urgent = 3,      // time-sensitive
    Critical = 4,    // safety-related
}

/// A message between agents
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message {
    pub id: u64,
    pub from: String,
    pub to: String,
    pub intent: Intent,
    pub content: String,
    pub priority: MessagePriority,
    pub confidence: f64,
    pub timestamp: u64,
    pub in_reply_to: Option<u64>,
    pub conversation_id: Option<String>,
    pub energy_cost: f64,
    pub context: Vec<String>,
}

impl Message {
    pub fn new(from: &str, to: &str, intent: Intent, content: &str) -> Self {
        let energy = match intent {
            Intent::Warn | Intent::Critical => 0.2,
            Intent::Command => 0.15,
            Intent::Request | Intent::Offer => 0.1,
            Intent::Inform | Intent::Query => 0.05,
            Intent::Accept | Intent::Reject => 0.03,
            Intent::Apologize | Intent::Thank => 0.01,
        };
        Message { id: 0, from: from.to_string(), to: to.to_string(), intent, content: content.to_string(), priority: MessagePriority::Normal, confidence: 0.8, timestamp: now(), in_reply_to: None, conversation_id: None, energy_cost: energy, context: vec![] }
    }

    pub fn with_priority(mut self, p: MessagePriority) -> Self { self.priority = p; self }
    pub fn with_reply(mut self, msg_id: u64) -> Self { self.in_reply_to = Some(msg_id); self }
    pub fn with_conversation(mut self, conv_id: &str) -> Self { self.conversation_id = Some(conv_id.to_string()); self }
}

/// Intent extraction from message content
pub fn extract_intent(text: &str) -> (Intent, f64) {
    let lower = text.to_lowercase();
    let scores: Vec<(Intent, f64)> = vec![
        (Intent::Inform, keyword_score(&lower, &["tell", "sharing", "update", "fyi", "note that"])),
        (Intent::Request, keyword_score(&lower, &["please", "could you", "would you", "need", "help me", "can you"])),
        (Intent::Command, keyword_score(&lower, &["do this", "go to", "stop", "move", "avoid", "must", "immediately"])),
        (Intent::Query, keyword_score(&lower, &["what", "where", "when", "how", "why", "which", "?"])),
        (Intent::Offer, keyword_score(&lower, &["i can", "would you like", "trade", "exchange", "offer"])),
        (Intent::Accept, keyword_score(&lower, &["yes", "ok", "sure", "agreed", "accept", "done"])),
        (Intent::Reject, keyword_score(&lower, &["no", "can't", "won't", "refuse", "decline", "unable"])),
        (Intent::Warn, keyword_score(&lower, &["warning", "danger", "caution", "watch out", "alert", "hazard"])),
        (Intent::Apologize, keyword_score(&lower, &["sorry", "apologize", "my fault", "mistake", "regret"])),
        (Intent::Thank, keyword_score(&lower, &["thanks", "thank you", "appreciate", "grateful"])),
    ];

    let best = scores.into_iter().max_by(|a, b| a.1.partial_cmp(&b.1).unwrap()).unwrap_or((Intent::Inform, 0.1));
    best
}

fn keyword_score(text: &str, keywords: &[&str]) -> f64 {
    let matches = keywords.iter().filter(|k| text.contains(k)).count();
    (matches as f64 / keywords.len() as f64).max(if text.contains("?") { 0.3 } else { 0.0 })
}

/// A conversation thread
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub participants: Vec<String>,
    pub messages: VecDeque<Message>,
    pub topic: String,
    pub max_messages: usize,
    pub turns: u32,
}

impl Conversation {
    pub fn new(id: &str, participants: Vec<String>) -> Self {
        Conversation { id: id.to_string(), participants, messages: VecDeque::new(), topic: String::new(), max_messages: 50, turns: 0 }
    }

    pub fn add(&mut self, mut msg: Message) {
        msg.conversation_id = Some(self.id.clone());
        if self.messages.len() >= self.max_messages { self.messages.pop_front(); }
        self.messages.push_back(msg);
        self.turns += 1;
        if self.topic.is_empty() && self.messages.len() >= 2 {
            self.topic = format!("{} & {}", self.messages[0].content.chars().take(20).collect::<String>(), self.messages[1].content.chars().take(20).collect::<String>());
        }
    }

    /// Get last N messages
    pub fn recent(&self, n: usize) -> Vec<&Message> {
        self.messages.iter().rev().take(n).rev().collect()
    }

    /// Is this agent a participant?
    pub fn has_participant(&self, agent: &str) -> bool {
        self.participants.iter().any(|p| p == agent)
    }
}

/// Communication budget — energy-limited messaging
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommunicationBudget {
    pub total_energy: f64,
    pub spent: f64,
    pub per_message_limit: f64,
    pub regenerate_rate: f64,  // per tick
}

impl CommunicationBudget {
    pub fn new(total: f64) -> Self { CommunicationBudget { total_energy: total, spent: 0.0, per_message_limit: 0.3, regenerate_rate: 0.05 } }

    pub fn can_send(&self, cost: f64) -> bool {
        (self.total_energy - self.spent) >= cost && cost <= self.per_message_limit
    }

    pub fn spend(&mut self, cost: f64) -> bool {
        if !self.can_send(cost) { return false; }
        self.spent += cost;
        true
    }

    pub fn remaining(&self) -> f64 { self.total_energy - self.spent }

    pub fn regenerate(&mut self) {
        self.spent = (self.spent - self.regenerate_rate).max(0.0);
    }
}

/// Shared vocabulary — common understanding between agents
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SharedVocabulary {
    pub terms: HashMap<String, f64>,  // term -> shared confidence
    pub negotiation_rounds: u32,
}

impl SharedVocabulary {
    pub fn new() -> Self { SharedVocabulary { terms: HashMap::new(), negotiation_rounds: 0 } }

    pub fn add_term(&mut self, term: &str, confidence: f64) {
        self.terms.insert(term.to_string(), confidence.clamp(0.0, 1.0));
    }

    pub fn knows(&self, term: &str) -> f64 {
        self.terms.get(term).copied().unwrap_or(0.0)
    }

    /// Grounding: check how much of a message is understood
    pub fn grounding_score(&self, message: &str) -> f64 {
        let words: Vec<&str> = message.split_whitespace().collect();
        if words.is_empty() { return 0.5; }
        let known: f64 = words.iter().filter(|w| self.terms.contains_key(*w)).count() as f64;
        known / words.len() as f64
    }

    /// Negotiate shared terms with another vocabulary
    pub fn negotiate(&mut self, other: &SharedVocabulary) -> Vec<String> {
        self.negotiation_rounds += 1;
        let mut new_terms = vec![];
        for (term, conf) in &other.terms {
            if !self.terms.contains_key(term) && *conf > 0.5 {
                self.terms.insert(term.clone(), *conf * 0.8); // slightly less confident from negotiation
                new_terms.push(term.clone());
            }
        }
        new_terms
    }
}

/// The communication engine
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommunicationEngine {
    pub inbox: VecDeque<Message>,
    pub outbox: VecDeque<Message>,
    pub conversations: HashMap<String, Conversation>,
    pub vocabulary: SharedVocabulary,
    pub budget: CommunicationBudget,
    pub next_msg_id: u64,
    pub messages_sent: u32,
    pub messages_received: u32,
}

impl CommunicationEngine {
    pub fn new() -> Self { CommunicationEngine { inbox: VecDeque::new(), outbox: VecDeque::new(), conversations: HashMap::new(), vocabulary: SharedVocabulary::new(), budget: CommunicationBudget::new(10.0), next_msg_id: 1, messages_sent: 0, messages_received: 0 } }

    /// Compose and queue a message
    pub fn compose(&mut self, from: &str, to: &str, intent: Intent, content: &str) -> Option<u64> {
        let mut msg = Message::new(from, to, intent, content);
        msg.id = self.next_msg_id;
        self.next_msg_id += 1;
        if !self.budget.spend(msg.energy_cost) { return None; }
        self.outbox.push_back(msg.clone());
        self.messages_sent += 1;
        Some(msg.id)
    }

    /// Receive a message
    pub fn receive(&mut self, msg: Message) {
        self.inbox.push_back(msg);
        self.messages_received += 1;
        self.budget.regenerate();
    }

    /// Pop next message from inbox
    pub fn next_incoming(&mut self) -> Option<Message> { self.inbox.pop_front() }

    /// Pop next message from outbox
    pub fn next_outgoing(&mut self) -> Option<Message> { self.outbox.pop_front() }

    /// Start or continue a conversation
    pub fn conversation(&mut self, conv_id: &str) -> Option<&mut Conversation> {
        if !self.conversations.contains_key(conv_id) { return None; }
        self.conversations.get_mut(conv_id)
    }

    /// Start new conversation
    pub fn start_conversation(&mut self, conv_id: &str, participants: Vec<String>) {
        self.conversations.insert(conv_id.to_string(), Conversation::new(conv_id, participants));
    }

    /// Summary
    pub fn summary(&self) -> String {
        format!("comm: sent={} recv={} budget={:.2} vocab={} convs={}", self.messages_sent, self.messages_received, self.budget.remaining(), self.vocabulary.terms.len(), self.conversations.len())
    }
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intent_extraction() {
        let (intent, _) = extract_intent("please help me navigate");
        assert_eq!(intent, Intent::Request);
    }

    #[test]
    fn test_intent_command() {
        let (intent, _) = extract_intent("stop immediately!");
        assert_eq!(intent, Intent::Command);
    }

    #[test]
    fn test_intent_query() {
        let (intent, _) = extract_intent("what is the distance?");
        assert_eq!(intent, Intent::Query);
    }

    #[test]
    fn test_intent_warn() {
        let (intent, _) = extract_intent("warning: danger ahead");
        assert_eq!(intent, Intent::Warn);
    }

    #[test]
    fn test_compose_send() {
        let mut engine = CommunicationEngine::new();
        let id = engine.compose("alice", "bob", Intent::Inform, "status ok");
        assert!(id.is_some());
        assert_eq!(engine.messages_sent, 1);
    }

    #[test]
    fn test_budget_limit() {
        let mut engine = CommunicationEngine::new();
        engine.budget.total_energy = 0.01; // very low
        let id = engine.compose("a", "b", Intent::Command, "go"); // costs 0.15
        assert!(id.is_none()); // can't afford
    }

    #[test]
    fn test_receive_process() {
        let mut engine = CommunicationEngine::new();
        let msg = Message::new("bob", "alice", Intent::Inform, "hello");
        engine.receive(msg);
        assert_eq!(engine.messages_received, 1);
        let incoming = engine.next_incoming();
        assert!(incoming.is_some());
    }

    #[test]
    fn test_conversation() {
        let mut engine = CommunicationEngine::new();
        engine.start_conversation("c1", vec!["alice".into(), "bob".into()]);
        let conv = engine.conversation("c1").unwrap();
        assert!(conv.has_participant("alice"));
    }

    #[test]
    fn test_conversation_add() {
        let mut engine = CommunicationEngine::new();
        engine.start_conversation("c1", vec!["a".into()]);
        engine.conversation("c1").unwrap().add(Message::new("a", "b", Intent::Inform, "hi"));
        engine.conversation("c1").unwrap().add(Message::new("b", "a", Intent::Accept, "ok"));
        let conv = engine.conversation("c1").unwrap();
        assert_eq!(conv.turns, 2);
        assert!(!conv.topic.is_empty());
    }

    #[test]
    fn test_vocabulary_grounding() {
        let mut vocab = SharedVocabulary::new();
        vocab.add_term("navigate", 0.9);
        vocab.add_term("left", 0.8);
        let score = vocab.grounding_score("navigate left");
        assert_eq!(score, 1.0);
        let score2 = vocab.grounding_score("navigate right up");
        assert!(score2 < 1.0);
    }

    #[test]
    fn test_vocabulary_negotiate() {
        let mut a = SharedVocabulary::new();
        let mut b = SharedVocabulary::new();
        a.add_term("move", 0.9);
        b.add_term("go", 0.9);
        let new = b.negotiate(&a);
        assert!(new.contains(&"move".to_string()));
        assert!(b.knows("move") > 0.0);
    }

    #[test]
    fn test_budget_regenerate() {
        let mut budget = CommunicationBudget::new(1.0);
        budget.spend(0.5);
        budget.regenerate();
        assert!(budget.remaining() > 0.5);
    }

    #[test]
    fn test_message_energy() {
        let cmd = Message::new("a", "b", Intent::Command, "go");
        let info = Message::new("a", "b", Intent::Inform, "hi");
        assert!(cmd.energy_cost > info.energy_cost);
    }
}

//! Coffres des propriétés : le jeu publie chaque dépôt et retrait dans un salon
//! Discord (webhook « Coffre propriété ») ; le bot lit ces messages, en déduit
//! le stock et l'historique par joueur, consultables par les gradés et leads.

pub mod commands;
pub mod ingest;
pub mod parser;
pub mod service;

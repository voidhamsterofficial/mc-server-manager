//! Pure parsing of Minecraft server log lines: state-change signals, log
//! levels, and ANSI / `§` color codes. No I/O here — everything is
//! unit-testable string inspection.

use serde::Serialize;

/// Severity extracted from a log line, used by the UI for coloring.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

/// A run of text with one style, extracted from color codes in the line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleSpan {
    pub text: String,
    /// CSS hex color, or `None` for the level's default color.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    pub bold: bool,
}

/// One console line as shown to the user.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleLine {
    pub spans: Vec<ConsoleSpan>,
    pub level: LogLevel,
}

/// Meaningful state changes recognised in the log stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsoleSignal {
    /// The server finished booting ("Done (3.14s)! ...").
    ServerReady,
    PlayerJoined(String),
    PlayerLeft(String),
    /// "Kicked <name>: <reason>" — logged when a player is kicked.
    PlayerKicked(String),
    /// `<Name> message` — a chat message.
    ChatMessage {
        player: String,
        message: String,
    },
    /// "Set <name>'s game mode to <Mode> Mode".
    GameModeChanged {
        player: String,
        mode: String,
    },
}

/// Parses one raw output line into its display form and any state-change
/// signal it carries. Signals are matched against the color-stripped text.
/// Lines that carry no color codes of their own (vanilla's piped output is
/// completely plain) get gentle semantic highlighting instead.
pub fn analyze(raw_line: &str) -> (ConsoleLine, Option<ConsoleSignal>) {
    let mut spans = parse_spans(raw_line);
    let plain_text: String = spans.iter().map(|span| span.text.as_str()).collect();

    let signal = parse_signal(&plain_text);
    let level = parse_log_level(&plain_text);

    if is_unstyled(&spans) {
        spans = highlight_plain(&plain_text, level);
    }

    let line = ConsoleLine { spans, level };
    (line, signal)
}

/// Vanilla logs `[HH:MM:SS] [Server thread/INFO]:`; Paper-family logs
/// `[HH:MM:SS INFO]:` — cover both bracket shapes.
pub fn parse_log_level(line: &str) -> LogLevel {
    let is_error = line.contains("ERROR]")
        || line.contains("FATAL]")
        || line.starts_with("Error")
        || line.contains("SEVERE]");
    if is_error {
        return LogLevel::Error;
    }

    let is_warning = line.contains("WARN]") || line.starts_with("WARNING");
    if is_warning {
        return LogLevel::Warn;
    }
    LogLevel::Info
}

/// Extracts a state-change signal from a log line, if it contains one.
pub fn parse_signal(line: &str) -> Option<ConsoleSignal> {
    // Bedrock's log format has no "]: " marker; its signals are literal.
    if line.contains("Server started.") {
        return Some(ConsoleSignal::ServerReady);
    }
    if let Some(player_name) = bedrock_player_name(line, "Player connected: ") {
        return Some(ConsoleSignal::PlayerJoined(player_name));
    }
    if let Some(player_name) = bedrock_player_name(line, "Player disconnected: ") {
        return Some(ConsoleSignal::PlayerLeft(player_name));
    }

    let message = message_body(line)?;

    if is_ready_message(message) {
        return Some(ConsoleSignal::ServerReady);
    }
    if let Some(player_name) = player_event_name(message, " joined the game") {
        return Some(ConsoleSignal::PlayerJoined(player_name));
    }
    if let Some(player_name) = player_event_name(message, " left the game") {
        return Some(ConsoleSignal::PlayerLeft(player_name));
    }
    if let Some(player_name) = kicked_player_name(message) {
        return Some(ConsoleSignal::PlayerKicked(player_name));
    }
    if let Some((player, mode)) = game_mode_change(message) {
        return Some(ConsoleSignal::GameModeChanged { player, mode });
    }
    if let Some((player, chat)) = chat_message(message) {
        return Some(ConsoleSignal::ChatMessage {
            player,
            message: chat,
        });
    }

    None
}

/// Detects a game-mode change and returns the affected player plus the short
/// mode word (Survival, Creative, Adventure, Spectator). Handles all the shapes
/// Minecraft emits:
/// * console targeting a player: `Set <name>'s game mode to <Mode> Mode`
/// * op broadcast, self:   `[<actor>: Set own game mode to <Mode> Mode]`
/// * op broadcast, other:  `[<actor>: Set <name>'s game mode to <Mode> Mode]`
fn game_mode_change(message: &str) -> Option<(String, String)> {
    // Command feedback broadcast to ops is wrapped as `[<actor>: <feedback>]`.
    let (actor, feedback) = match message
        .strip_prefix('[')
        .and_then(|inner| inner.strip_suffix(']'))
    {
        Some(wrapped) => {
            let (actor, feedback) = wrapped.split_once(": ")?;
            (Some(actor), feedback)
        }
        None => (None, message),
    };

    let after_prefix = feedback.strip_prefix("Set ")?;

    // "Set own game mode to X Mode" — the affected player is the actor.
    if let Some(rest) = after_prefix.strip_prefix("own game mode to ") {
        let player = actor.filter(|name| is_single_token(name))?;
        let mode = short_mode(rest)?;
        return Some((player.to_string(), mode));
    }

    // "Set <name>'s game mode to X Mode" — the affected player is <name>.
    let (name, rest) = after_prefix.split_once("'s game mode to ")?;
    if !is_single_token(name) {
        return None;
    }
    let mode = short_mode(rest)?;
    Some((name.to_string(), mode))
}

/// Trims the trailing " Mode" and whitespace, returning None if nothing is left.
fn short_mode(rest: &str) -> Option<String> {
    let mode = rest.trim_end_matches(" Mode").trim();
    if mode.is_empty() {
        return None;
    }
    Some(mode.to_string())
}

/// A single-token player name: non-empty, no spaces, no `<` (rules out chat).
fn is_single_token(name: &str) -> bool {
    !name.is_empty() && !name.contains(' ') && !name.contains('<')
}

/// Matches chat lines of the shape `<Name> message`.
fn chat_message(message: &str) -> Option<(String, String)> {
    let after_open = message.strip_prefix('<')?;
    let (name, rest) = after_open.split_once("> ")?;

    if !is_single_token(name) {
        return None;
    }
    Some((name.to_string(), rest.to_string()))
}

/// Bedrock logs `... Player connected: <gamertag>, xuid: <id>` — gamertags
/// may contain spaces, so the name runs until the comma.
fn bedrock_player_name(line: &str, marker: &str) -> Option<String> {
    let marker_position = line.find(marker)?;
    let after_marker = &line[marker_position + marker.len()..];

    let name = after_marker.split(',').next()?.trim();
    if name.is_empty() {
        return None;
    }
    Some(name.to_string())
}

/// Matches `Kicked <name>: <reason>` where the name is a single token.
fn kicked_player_name(message: &str) -> Option<String> {
    let after_prefix = message.strip_prefix("Kicked ")?;
    let (name, _reason) = after_prefix.split_once(':')?;
    if !is_single_token(name) {
        return None;
    }
    Some(name.to_string())
}

/// Returns the message portion after the first `]: ` marker, e.g.
/// `[12:00:00] [Server thread/INFO]: Done (3.1s)!` -> `Done (3.1s)!`.
///
/// Anchors on the first marker (which closes the log-level prefix) rather than
/// the last, so a chat message that itself contains `]: ` is returned intact
/// instead of being truncated to whatever follows its own `]: `.
fn message_body(line: &str) -> Option<&str> {
    let marker_position = line.find("]: ")?;
    let body = &line[marker_position + 3..];
    Some(body)
}

fn is_ready_message(message: &str) -> bool {
    let game_server_ready = message.starts_with("Done (") && message.contains(")!");
    // BungeeCord (and some proxies) report readiness by announcing their
    // listener instead of a "Done" line.
    let proxy_ready = message.starts_with("Listening on ");
    game_server_ready || proxy_ready
}

/// Matches messages of the exact shape `<name><suffix>` where the name is a
/// single token. Chat lines (`<Alex> hi`) never match because usernames
/// cannot contain `<` or spaces.
fn player_event_name(message: &str, suffix: &str) -> Option<String> {
    let name = message.strip_suffix(suffix)?;
    if !is_single_token(name) {
        return None;
    }
    Some(name.to_string())
}

// --- Semantic highlighting for plain lines -------------------------------

const TIMESTAMP_COLOR: &str = "#8a8a92";
const INFO_TAG_COLOR: &str = "#71d95c";
const WARN_TAG_COLOR: &str = "#ffaa00";
const ERROR_TAG_COLOR: &str = "#ff5555";

fn is_unstyled(spans: &[ConsoleSpan]) -> bool {
    let has_style = spans.iter().any(|span| span.color.is_some() || span.bold);
    !has_style
}

/// Colors the log prefix of standard lines so plain output still reads well:
/// vanilla's `[time] [thread/LEVEL]:` and Paper's `[time LEVEL]:` shapes are
/// both handled. The message body keeps the line-level default color.
fn highlight_plain(plain_text: &str, level: LogLevel) -> Vec<ConsoleSpan> {
    let mut spans: Vec<ConsoleSpan> = Vec::new();
    let mut remaining = plain_text;

    if let Some((first_tag, after_first)) = split_bracket_prefix(remaining) {
        // Paper folds the level into the first bracket; vanilla keeps a
        // plain timestamp there.
        let first_color = if contains_level_word(first_tag) {
            level_tag_color(level)
        } else {
            TIMESTAMP_COLOR
        };
        spans.push(plain_span(first_tag, Some(first_color)));
        remaining = after_first;

        if let Some((thread_tag, after_tag)) = split_bracket_prefix(remaining) {
            spans.push(plain_span(thread_tag, Some(level_tag_color(level))));
            remaining = after_tag;
        }
    }

    if !remaining.is_empty() {
        spans.push(plain_span(remaining, None));
    }
    spans
}

fn contains_level_word(text: &str) -> bool {
    const LEVEL_WORDS: [&str; 6] = ["INFO]", "WARN]", "ERROR]", "FATAL]", "DEBUG]", "TRACE]"];
    LEVEL_WORDS.iter().any(|word| text.contains(word))
}

fn plain_span(text: &str, color: Option<&str>) -> ConsoleSpan {
    ConsoleSpan {
        text: text.to_string(),
        color: color.map(str::to_string),
        bold: false,
    }
}

fn level_tag_color(level: LogLevel) -> &'static str {
    match level {
        LogLevel::Info => INFO_TAG_COLOR,
        LogLevel::Warn => WARN_TAG_COLOR,
        LogLevel::Error => ERROR_TAG_COLOR,
    }
}

/// Splits `  [bracketed] rest` into the bracketed prefix (leading whitespace
/// included) and the remainder.
fn split_bracket_prefix(text: &str) -> Option<(&str, &str)> {
    let content = text.trim_start();
    if !content.starts_with('[') {
        return None;
    }

    let leading_length = text.len() - content.len();
    let close_position = content.find(']')?;
    let split_at = leading_length + close_position + 1;
    Some((&text[..split_at], &text[split_at..]))
}

// --- Color code parsing -------------------------------------------------

/// Minecraft's classic `§0`–`§f` palette.
const MINECRAFT_COLORS: [&str; 16] = [
    "#000000", "#0000AA", "#00AA00", "#00AAAA", "#AA0000", "#AA00AA", "#FFAA00", "#AAAAAA",
    "#555555", "#5555FF", "#55FF55", "#55FFFF", "#FF5555", "#FF55FF", "#FFFF55", "#FFFFFF",
];

/// ANSI SGR 30–37, matched to the Minecraft palette for a consistent look.
const ANSI_BASIC_COLORS: [&str; 8] = [
    "#555555", "#AA0000", "#00AA00", "#FFAA00", "#5555FF", "#AA00AA", "#00AAAA", "#AAAAAA",
];

/// ANSI SGR 90–97 (bright variants).
const ANSI_BRIGHT_COLORS: [&str; 8] = [
    "#555555", "#FF5555", "#55FF55", "#FFFF55", "#5555FF", "#FF55FF", "#55FFFF", "#FFFFFF",
];

struct SpanBuilder {
    spans: Vec<ConsoleSpan>,
    buffer: String,
    color: Option<String>,
    bold: bool,
}

impl SpanBuilder {
    fn new() -> Self {
        Self {
            spans: Vec::new(),
            buffer: String::new(),
            color: None,
            bold: false,
        }
    }

    fn push_char(&mut self, character: char) {
        self.buffer.push(character);
    }

    fn set_color(&mut self, color: Option<String>) {
        self.flush();
        self.color = color;
    }

    fn set_bold(&mut self, bold: bool) {
        self.flush();
        self.bold = bold;
    }

    fn reset_style(&mut self) {
        self.flush();
        self.color = None;
        self.bold = false;
    }

    fn flush(&mut self) {
        if self.buffer.is_empty() {
            return;
        }
        let span = ConsoleSpan {
            text: std::mem::take(&mut self.buffer),
            color: self.color.clone(),
            bold: self.bold,
        };
        self.spans.push(span);
    }

    fn finish(mut self) -> Vec<ConsoleSpan> {
        self.flush();
        self.spans
    }
}

/// Splits a raw line into styled spans, stripping ANSI escape sequences and
/// Minecraft `§` codes from the text.
pub fn parse_spans(raw_line: &str) -> Vec<ConsoleSpan> {
    let mut builder = SpanBuilder::new();
    let mut characters = raw_line.chars().peekable();

    while let Some(character) = characters.next() {
        if character == '\u{1b}' {
            consume_ansi_sequence(&mut characters, &mut builder);
            continue;
        }
        if character == '§' {
            consume_section_code(&mut characters, &mut builder);
            continue;
        }
        builder.push_char(character);
    }

    builder.finish()
}

type CharStream<'a> = std::iter::Peekable<std::str::Chars<'a>>;

/// Consumes an ANSI escape sequence; SGR (`...m`) sequences update the
/// style, anything else is dropped from the output.
fn consume_ansi_sequence(characters: &mut CharStream, builder: &mut SpanBuilder) {
    if characters.peek() != Some(&'[') {
        return;
    }
    characters.next();

    let mut parameter_text = String::new();
    for character in characters.by_ref() {
        if character == 'm' {
            apply_sgr_parameters(&parameter_text, builder);
            return;
        }
        let is_parameter_char = character.is_ascii_digit() || character == ';';
        if !is_parameter_char {
            // Not an SGR sequence (cursor movement etc.) — drop it entirely.
            return;
        }
        parameter_text.push(character);
    }
}

fn apply_sgr_parameters(parameter_text: &str, builder: &mut SpanBuilder) {
    let numbers: Vec<u32> = parameter_text
        .split(';')
        .filter_map(|part| part.parse().ok())
        .collect();
    // An empty parameter list (`\x1b[m`) means reset.
    if numbers.is_empty() {
        builder.reset_style();
        return;
    }

    let mut index = 0;
    while index < numbers.len() {
        index += apply_one_sgr(&numbers[index..], builder);
    }
}

/// Applies the SGR code at the start of `numbers`, returning how many
/// parameters it consumed.
fn apply_one_sgr(numbers: &[u32], builder: &mut SpanBuilder) -> usize {
    let code = numbers[0];
    match code {
        0 => builder.reset_style(),
        1 => builder.set_bold(true),
        22 => builder.set_bold(false),
        39 => builder.set_color(None),
        30..=37 => builder.set_color(Some(ANSI_BASIC_COLORS[(code - 30) as usize].to_string())),
        90..=97 => builder.set_color(Some(ANSI_BRIGHT_COLORS[(code - 90) as usize].to_string())),
        38 => {
            return apply_extended_color(numbers, builder);
        }
        _ => {}
    }
    1
}

/// Handles `38;5;<index>` (256-color) and `38;2;<r>;<g>;<b>` (truecolor).
fn apply_extended_color(numbers: &[u32], builder: &mut SpanBuilder) -> usize {
    if numbers.get(1) == Some(&5) {
        if let Some(&palette_index) = numbers.get(2) {
            builder.set_color(Some(xterm_256_to_hex(palette_index)));
        }
        return 3;
    }
    if numbers.get(1) == Some(&2) && numbers.len() >= 5 {
        // SGR values are 8-bit; clamp so out-of-range input can't produce a
        // malformed hex string with more than two digits per channel.
        let red = numbers[2].min(255);
        let green = numbers[3].min(255);
        let blue = numbers[4].min(255);
        let color = format!("#{red:02X}{green:02X}{blue:02X}");
        builder.set_color(Some(color));
        return 5;
    }
    2
}

/// Standard xterm 256-color palette to hex.
fn xterm_256_to_hex(palette_index: u32) -> String {
    if palette_index < 8 {
        return ANSI_BASIC_COLORS[palette_index as usize].to_string();
    }
    if palette_index < 16 {
        return ANSI_BRIGHT_COLORS[(palette_index - 8) as usize].to_string();
    }
    if palette_index < 232 {
        let cube_index = palette_index - 16;
        let red = cube_component(cube_index / 36);
        let green = cube_component((cube_index / 6) % 6);
        let blue = cube_component(cube_index % 6);
        return format!("#{red:02X}{green:02X}{blue:02X}");
    }

    let gray = 8 + (palette_index.min(255) - 232) * 10;
    format!("#{gray:02X}{gray:02X}{gray:02X}")
}

fn cube_component(step: u32) -> u32 {
    if step == 0 {
        return 0;
    }
    55 + step * 40
}

fn consume_section_code(characters: &mut CharStream, builder: &mut SpanBuilder) {
    let Some(code) = characters.next() else {
        return;
    };

    let lowered = code.to_ascii_lowercase();
    if let Some(palette_index) = lowered.to_digit(16) {
        let color = MINECRAFT_COLORS[palette_index as usize].to_string();
        builder.set_color(Some(color));
        return;
    }
    match lowered {
        'l' => builder.set_bold(true),
        'r' => builder.reset_style(),
        // Other formatting codes (italic, obfuscated, ...) are stripped.
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const READY_LINE: &str =
        r#"[12:34:56] [Server thread/INFO]: Done (3.141s)! For help, type "help""#;

    #[test]
    fn detects_server_ready() {
        let (_, signal) = analyze(READY_LINE);
        assert_eq!(signal, Some(ConsoleSignal::ServerReady));
    }

    #[test]
    fn detects_player_join_and_leave() {
        let joined = parse_signal("[12:00:00] [Server thread/INFO]: Alex joined the game");
        assert_eq!(
            joined,
            Some(ConsoleSignal::PlayerJoined("Alex".to_string()))
        );

        let left = parse_signal("[12:00:00] [Server thread/INFO]: Alex left the game");
        assert_eq!(left, Some(ConsoleSignal::PlayerLeft("Alex".to_string())));
    }

    #[test]
    fn chat_that_mimics_events_is_chat_not_a_join() {
        // A chat line must never be misread as a join/leave; it's chat.
        let chat_line = "[12:00:00] [Server thread/INFO]: <Alex> somebody joined the game";
        assert_eq!(
            parse_signal(chat_line),
            Some(ConsoleSignal::ChatMessage {
                player: "Alex".to_string(),
                message: "somebody joined the game".to_string(),
            })
        );
    }

    #[test]
    fn detects_game_mode_changes() {
        let line = "[12:00:00] [Server thread/INFO]: Set Alex's game mode to Creative Mode";
        assert_eq!(
            parse_signal(line),
            Some(ConsoleSignal::GameModeChanged {
                player: "Alex".to_string(),
                mode: "Creative".to_string(),
            })
        );
    }

    #[test]
    fn detects_self_game_mode_change_broadcast() {
        // A player changing their own mode is broadcast to ops as
        // `[<actor>: Set own game mode to ...]` — the actor is the player.
        let line = "[12:00:00] [Server thread/INFO]: [Alex: Set own game mode to Survival Mode]";
        assert_eq!(
            parse_signal(line),
            Some(ConsoleSignal::GameModeChanged {
                player: "Alex".to_string(),
                mode: "Survival".to_string(),
            })
        );
    }

    #[test]
    fn detects_op_game_mode_change_broadcast() {
        // An op changing someone else's mode: the affected player is the name,
        // not the acting op.
        let line = "[12:00:00] [Server thread/INFO]: [Bob: Set Alex's game mode to Adventure Mode]";
        assert_eq!(
            parse_signal(line),
            Some(ConsoleSignal::GameModeChanged {
                player: "Alex".to_string(),
                mode: "Adventure".to_string(),
            })
        );
    }

    #[test]
    fn captures_plain_chat() {
        let chat_line = "[12:00:00] [Server thread/INFO]: <Alex> hello world";
        assert_eq!(
            parse_signal(chat_line),
            Some(ConsoleSignal::ChatMessage {
                player: "Alex".to_string(),
                message: "hello world".to_string(),
            })
        );
    }

    #[test]
    fn detects_bedrock_signals() {
        let ready = "[2026-07-17 00:00:00:000 INFO] Server started.";
        assert_eq!(parse_signal(ready), Some(ConsoleSignal::ServerReady));

        let joined = "[2026-07-17 INFO] Player connected: Cool Gamertag 42, xuid: 2535423";
        assert_eq!(
            parse_signal(joined),
            Some(ConsoleSignal::PlayerJoined("Cool Gamertag 42".to_string()))
        );

        let left = "[2026-07-17 INFO] Player disconnected: Cool Gamertag 42, xuid: 2535423";
        assert_eq!(
            parse_signal(left),
            Some(ConsoleSignal::PlayerLeft("Cool Gamertag 42".to_string()))
        );
    }

    #[test]
    fn detects_player_kicks() {
        let kicked =
            parse_signal("[12:00:00] [Server thread/INFO]: Kicked Alex: Kicked by an operator");
        assert_eq!(
            kicked,
            Some(ConsoleSignal::PlayerKicked("Alex".to_string()))
        );

        // A chat line that mentions "Kicked" is chat, not a real kick.
        let chat = "[12:00:00] [Server thread/INFO]: <Bob> Kicked Alex: just kidding";
        assert_eq!(
            parse_signal(chat),
            Some(ConsoleSignal::ChatMessage {
                player: "Bob".to_string(),
                message: "Kicked Alex: just kidding".to_string(),
            })
        );
    }

    #[test]
    fn classifies_log_levels() {
        assert_eq!(
            parse_log_level("[12:00:00] [Server thread/WARN]: low memory"),
            LogLevel::Warn
        );
        assert_eq!(
            parse_log_level("[12:00:00] [Server thread/ERROR]: boom"),
            LogLevel::Error
        );
        assert_eq!(parse_log_level(READY_LINE), LogLevel::Info);
    }

    #[test]
    fn plain_text_becomes_one_unstyled_span() {
        let spans = parse_spans("hello world");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].text, "hello world");
        assert_eq!(spans[0].color, None);
        assert!(!spans[0].bold);
    }

    #[test]
    fn parses_ansi_colors_and_reset() {
        let spans = parse_spans("\u{1b}[31mred\u{1b}[0m plain");
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].text, "red");
        assert_eq!(spans[0].color.as_deref(), Some("#AA0000"));
        assert_eq!(spans[1].text, " plain");
        assert_eq!(spans[1].color, None);
    }

    #[test]
    fn parses_256_color_and_bold() {
        let spans = parse_spans("\u{1b}[1;38;5;10mbright\u{1b}[22m dim");
        assert_eq!(spans[0].text, "bright");
        assert!(spans[0].bold);
        assert_eq!(spans[0].color.as_deref(), Some("#55FF55"));
        assert!(!spans[1].bold);
    }

    #[test]
    fn parses_minecraft_section_codes() {
        let spans = parse_spans("§aGreen§r and §lBold");
        assert_eq!(spans[0].text, "Green");
        assert_eq!(spans[0].color.as_deref(), Some("#55FF55"));
        assert_eq!(spans[1].text, " and ");
        assert_eq!(spans[1].color, None);
        assert_eq!(spans[2].text, "Bold");
        assert!(spans[2].bold);
    }

    #[test]
    fn signals_are_detected_through_color_codes() {
        let colored_ready =
            "\u{1b}[32m[12:34:56] [Server thread/INFO]: Done (3.1s)! ready\u{1b}[0m";
        let (line, signal) = analyze(colored_ready);
        assert_eq!(signal, Some(ConsoleSignal::ServerReady));
        assert_eq!(line.level, LogLevel::Info);
    }

    #[test]
    fn plain_log_lines_get_semantic_highlighting() {
        let (line, _) = analyze("[00:02:08] [Server thread/INFO]: Loading properties");
        assert_eq!(line.spans.len(), 3);
        assert_eq!(line.spans[0].text, "[00:02:08]");
        assert_eq!(line.spans[0].color.as_deref(), Some(TIMESTAMP_COLOR));
        assert_eq!(line.spans[1].text, " [Server thread/INFO]");
        assert_eq!(line.spans[1].color.as_deref(), Some(INFO_TAG_COLOR));
        assert_eq!(line.spans[2].text, ": Loading properties");
        assert_eq!(line.spans[2].color, None);
    }

    #[test]
    fn jvm_warnings_are_classified_as_warnings() {
        let (line, _) = analyze("WARNING: A restricted method in java.lang.System has been called");
        assert_eq!(line.level, LogLevel::Warn);
        assert_eq!(line.spans.len(), 1);
        assert_eq!(line.spans[0].color, None);
    }

    #[test]
    fn colored_lines_skip_semantic_highlighting() {
        let (line, _) = analyze("\u{1b}[31m[00:00:00] [Server thread/INFO]: red\u{1b}[0m");
        assert_eq!(line.spans.len(), 1);
        assert_eq!(line.spans[0].color.as_deref(), Some("#AA0000"));
    }
}

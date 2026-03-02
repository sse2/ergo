use std::collections::VecDeque;
use std::process::Command;

use log::*;

#[derive(Debug, PartialEq, Clone)]
pub enum Condition {
    DisplayName(String),
    NumDisplays(usize),
    MoreThanNumDisplays(usize),
    LessThanNumDisplays(usize),
    AddedDisplay,
    RemovedDisplay,

    And,
    Or,
    Not,
    None,
}

#[derive(Debug, Clone)]
pub struct ConditionNode {
    pub condition: Condition,
    pub left: Option<Box<ConditionNode>>,
    pub right: Option<Box<ConditionNode>>,
}

impl ConditionNode {
    pub fn debug_print(&self, level: usize) {
        let indent = "    ".repeat(level);
        let condition_str = format!("{:?}", self.condition);

        debug!("{}{}", indent, condition_str);

        if let Some(ref left) = self.left {
            left.debug_print(level + 1);
        }
        if let Some(ref right) = self.right {
            right.debug_print(level + 1);
        }
    }
}

pub struct DisplayState {
    pub current_displays: Vec<String>,
    pub added: bool,
    pub removed: bool,
}

fn parse_condition(condition: &str) -> Condition {
    let trimmed = condition.trim();

    if trimmed.starts_with("=") {
        Condition::NumDisplays(trimmed[1..].trim().parse::<usize>().unwrap_or(0xffffffff))
    } else if trimmed.starts_with(">") {
        Condition::MoreThanNumDisplays(trimmed[1..].trim().parse::<usize>().unwrap_or(0xffffffff))
    } else if trimmed.starts_with("<") {
        Condition::LessThanNumDisplays(trimmed[1..].trim().parse::<usize>().unwrap_or(0))
    } else if trimmed == "+" {
        Condition::AddedDisplay
    } else if trimmed == "-" {
        Condition::RemovedDisplay
    } else if trimmed == "and" {
        Condition::And
    } else if trimmed == "or" {
        Condition::Or
    } else if trimmed.starts_with("\"") {
        if trimmed.ends_with("\"") {
            Condition::DisplayName(trimmed[1..trimmed.len() - 1].to_string())
        } else {
            Condition::DisplayName(trimmed[1..].to_string()) // allow anyway if they forget to close the quote - for now
        }
    } else if !trimmed.is_empty() {
        Condition::DisplayName(trimmed.to_string())
    } else {
        Condition::None
    }
}

fn split_conditions(conditions: &str) -> Vec<Condition> {
    let mut result = Vec::new();
    let mut current_condition = String::new();
    let mut inside_quotes = false;

    let chars = conditions.chars().collect::<Vec<char>>();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];

        if c == '"' {
            inside_quotes = !inside_quotes;
            current_condition.push(c);
        } else if c.is_whitespace() && !inside_quotes {
            if !current_condition.is_empty() {
                result.push(parse_condition(&current_condition));
                current_condition.clear();
            }
        } else {
            current_condition.push(c);
        }

        i += 1;
    }

    if !current_condition.is_empty() {
        result.push(parse_condition(&current_condition));
    }

    result
}

// [condition] : [command]
// all possible conditions are:
// 1. + - command will be executed when a display is connected
// 2. - - command will be executed when a display is disconnected
// 3. "displayname" - command will be executed when display named "displayname" is present - displayname matches what you see inside System Preferences -> Displays
// 4. =2 - command will be executed when there are 2 displays connected, where 2 can be replaced with any number
// 5. >2 - command will be executed when there are more than 2 displays connected, where 2 can be replaced with any number
// 6. <2 - command will be executed when there are less than 2 displays connected, where 2 can be replaced with any number
// of note: conditions may be combined with 'and' and 'or' operators
// example: + and "Built-in" and =2 -> echo 'hello'
pub fn parse_rule(rule: String) -> Result<(ConditionNode, String), String> {
    // check if we're a comment
    if rule.trim().starts_with("#") {
        return Ok((
            ConditionNode {
                condition: Condition::None,
                left: None,
                right: None,
            },
            "".to_string(),
        ));
    }

    let substrs = rule.split("->");

    // handle error
    if substrs.clone().count() != 2 {
        return Err("invalid rule format".to_string());
    }

    let mut conditions = split_conditions(substrs.clone().next().unwrap().trim());

    // operators need to go to the back of the conditions vector here, but in the same order they were found
    let operators: Vec<Condition> = conditions
        .iter()
        .filter(|c| **c == Condition::And || **c == Condition::Or || **c == Condition::Not)
        .cloned()
        .rev()
        .collect();
    conditions.retain(|c| *c != Condition::And && *c != Condition::Or && *c != Condition::Not);
    conditions.reverse();
    conditions.extend(operators);

    let mut stack = VecDeque::new();
    // a condition tree, where we handle 'and' and 'or' operators.
    // we don't handle operator precedence, so 'and' and 'or' have the same precedence and are evaluated from left to right (first come first serve)
    // example:
    // input: + and "G27QC A" and =1 -> echo 'hello'
    // condition tree:
    //       and
    //      /   \
    //     +    and
    //         /    \
    //     "G27QC A"  =1
    for condition in conditions {
        if condition == Condition::And {
            let right = stack.pop_front().unwrap();
            let left = stack.pop_front().unwrap();

            stack.push_back(ConditionNode {
                condition: Condition::And,
                left: Some(Box::new(left)),
                right: Some(Box::new(right)),
            });
        } else if condition == Condition::Or {
            let right = stack.pop_front().unwrap();
            let left = stack.pop_front().unwrap();

            stack.push_back(ConditionNode {
                condition: Condition::Or,
                left: Some(Box::new(left)),
                right: Some(Box::new(right)),
            });
        } else {
            stack.push_back(ConditionNode {
                condition,
                left: None,
                right: None,
            });
        }
    }

    let tree = stack.pop_back().unwrap();

    // debug print condition tree
    tree.debug_print(0);

    let command = substrs.clone().last().unwrap().trim();
    debug!("command: {}", command);

    Ok((tree, command.to_string()))
}

pub fn is_condition_true(node: &ConditionNode, state: &DisplayState) -> bool {
    match &node.condition {
        Condition::DisplayName(name) => state.current_displays.iter().any(|d| d == name),
        Condition::NumDisplays(n) => state.current_displays.len() == *n,
        Condition::MoreThanNumDisplays(n) => state.current_displays.len() > *n,
        Condition::LessThanNumDisplays(n) => state.current_displays.len() < *n,
        Condition::AddedDisplay => state.added,
        Condition::RemovedDisplay => state.removed,
        Condition::And => {
            is_condition_true(node.left.as_ref().unwrap(), state)
                && is_condition_true(node.right.as_ref().unwrap(), state)
        }
        Condition::Or => {
            is_condition_true(node.left.as_ref().unwrap(), state)
                || is_condition_true(node.right.as_ref().unwrap(), state)
        }
        _ => false,
    }
}

pub fn try_execute_command(tree: &ConditionNode, command: &str, state: &DisplayState) {
    let should_exec = is_condition_true(tree, state);

    if should_exec {
        info!("running command: {}", command);

        let shell: String = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
        if let Err(e) = Command::new(shell).arg("-c").arg(command).status() {
            warn!("unable to execute command: {}", e);
        }
    }
}

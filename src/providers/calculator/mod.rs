use crate::app::SearchResult;
use crate::error::ElementError;
use crate::providers::{SearchContext, SearchProvider};

pub struct CalculatorProvider;

impl SearchProvider for CalculatorProvider {
    fn id(&self) -> &'static str {
        "calculator"
    }

    fn priority(&self) -> i32 {
        10
    }

    fn should_run(&self, query: &str) -> bool {
        query.chars().any(|c| {
            c.is_ascii_digit() || matches!(c, '+' | '-' | '*' | '/' | 'x' | '÷' | '(' | ')')
        })
    }

    fn search(&self, _ctx: &SearchContext, query: &str) -> Vec<SearchResult> {
        let expr = query.replace('x', "*").replace('÷', "/");
        if let Ok(val) = evalexpr::eval(&expr) {
            let output = val.to_string();
            vec![SearchResult {
                title: format!("= {output}"),
                subtitle: format!("Calc: {}", query),
                kind: "calc".into(),
                provider_id: "calculator".into(),
                action: output,
                icon_rgba: None,
                score: 1000.0,
            }]
        } else {
            Vec::new()
        }
    }

    fn activate(&self, _ctx: &SearchContext, result: &SearchResult) -> Result<(), ElementError> {
        arboard::Clipboard::new()
            .and_then(|mut c| c.set_text(&result.action))
            .map_err(|e| ElementError::Other(format!("clipboard error: {:?}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_math_expressions() {
        assert!(CalculatorProvider.should_run("1+1"));
        assert!(CalculatorProvider.should_run("2 * 3"));
        assert!(CalculatorProvider.should_run("(5+5)/2"));
        assert!(!CalculatorProvider.should_run("hello world"));
        assert!(!CalculatorProvider.should_run(""));
        assert!(!CalculatorProvider.should_run("notamath"));
    }
}

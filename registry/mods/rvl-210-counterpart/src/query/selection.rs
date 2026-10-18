//! Ordered query grammar and parse-entry validation.

use super::{Expression, Literal, MAX_DEPTH, MAX_LENGTH, MAX_STEPS, ParseError, Query, Step};

pub(super) fn parse(written: &str) -> Result<Query, ParseError> {
    if written.len() > MAX_LENGTH {
        return Err(ParseError::TooLong {
            limit: MAX_LENGTH,
            length: written.len(),
        });
    }
    let mut parser = Parser {
        written,
        at: 0,
        steps: 0,
    };
    let query = parser.query(0)?;
    if parser.at != written.len() {
        return Err(ParseError::NoHandler {
            at: parser.at,
            rest: parser.rest().chars().take(16).collect(),
        });
    }
    Ok(query)
}

struct Parser<'written> {
    written: &'written str,
    at: usize,
    steps: usize,
}

impl<'written> Parser<'written> {
    fn rest(&self) -> &'written str {
        &self.written[self.at..]
    }

    fn eat(&mut self, token: &str) -> bool {
        if self.rest().starts_with(token) {
            self.at += token.len();
            return true;
        }
        false
    }

    fn count_step(&mut self) -> Result<(), ParseError> {
        self.steps += 1;
        if self.steps > MAX_STEPS {
            return Err(ParseError::TooManySteps { limit: MAX_STEPS });
        }
        Ok(())
    }

    fn query(&mut self, depth: usize) -> Result<Query, ParseError> {
        if depth > MAX_DEPTH {
            return Err(ParseError::TooDeep {
                limit: MAX_DEPTH,
                at: self.at,
            });
        }
        let mut steps = Vec::new();
        while let Some(step) = self.step(depth)? {
            self.count_step()?;
            steps.push(step);
        }
        Ok(Query { steps })
    }

    /// Tries the position handlers in the published order and takes the first
    /// that matches. The comparison and logical handlers are infix or belong
    /// inside a filter, and are reached from [`Parser::expression`].
    fn step(&mut self, depth: usize) -> Result<Option<Step>, ParseError> {
        if let Some(step) = self.position()? {
            return Ok(Some(step));
        }
        self.nested_step(depth)
    }

    /// Reads the non-recursive position forms before the nested handlers.
    fn position(&mut self) -> Result<Option<Step>, ParseError> {
        let at = self.at;
        // 1 root
        if self.eat("$") {
            return Ok(Some(Step::Root));
        }
        // 2 length, before the named child so `.length` is not a child
        if self.eat(".length") {
            return Ok(Some(Step::Length));
        }
        // 3 named child
        if self.rest().starts_with('.') && !self.rest().starts_with(".*") {
            self.at += 1;
            let name = self.name();
            if name.is_empty() {
                self.at = at;
            } else {
                return Ok(Some(Step::Child(name)));
            }
        }
        // 4 named child in bracket form
        if self.rest().starts_with("['") {
            self.at += 2;
            let name = self.name();
            if self.eat("']") {
                return Ok(Some(Step::BracketChild(name)));
            }
            return Err(ParseError::Unclosed {
                opened: "bracket name",
                at,
            });
        }
        // 5 parent
        if self.eat("^") {
            return Ok(Some(Step::Parent));
        }
        // 6 all children
        if self.eat(".*") {
            return Ok(Some(Step::AllChildren));
        }
        // 7 current node
        if self.eat("@") {
            return Ok(Some(Step::Current));
        }
        Ok(None)
    }

    /// Reads brackets, grouping and filters after simple position forms fail.
    fn nested_step(&mut self, depth: usize) -> Result<Option<Step>, ParseError> {
        let at = self.at;
        // 8, 9 and 10 all open with a bracket and are told apart by what is in it
        if self.rest().starts_with('[') {
            return self.bracket(depth).map(Some);
        }
        // 11 parenthesised grouping
        if self.rest().starts_with('(') {
            self.at += 1;
            let inner = self.query(depth + 1)?;
            if !self.eat(")") {
                return Err(ParseError::Unclosed {
                    opened: "grouping",
                    at,
                });
            }
            return Ok(Some(Step::Group(Box::new(inner))));
        }
        // 22 filter expression
        if self.rest().starts_with("?(") {
            self.at += 2;
            let expression = self.expression(depth + 1)?;
            if !self.eat(")") {
                return Err(ParseError::Unclosed {
                    opened: "filter",
                    at,
                });
            }
            return Ok(Some(Step::Filter(Box::new(expression))));
        }
        Ok(None)
    }

    fn bracket(&mut self, depth: usize) -> Result<Step, ParseError> {
        let at = self.at;
        self.at += 1;
        let Some(end) = self.rest().find(']') else {
            return Err(ParseError::Unclosed {
                opened: "bracket",
                at,
            });
        };
        let inside = self.rest()[..end].to_owned();
        // 8 list element by position
        if let Ok(index) = inside.trim().parse::<i64>() {
            self.at += end + 1;
            return Ok(Step::Element(index));
        }
        // 9 list slice
        if let Some((from, to)) = inside.split_once(':')
            && !inside.contains('|')
        {
            let bound = |written: &str| -> Option<i64> { written.trim().parse::<i64>().ok() };
            self.at += end + 1;
            return Ok(Step::Slice(bound(from), bound(to)));
        }
        // 10 union of expressions
        if inside.contains('|') {
            let mut branches = Vec::new();
            for branch in inside.split('|') {
                let mut inner = Parser {
                    written: branch,
                    at: 0,
                    steps: 0,
                };
                let query = inner.query(depth + 1)?;
                if inner.at != branch.len() {
                    return Err(ParseError::NoHandler {
                        at,
                        rest: branch.to_owned(),
                    });
                }
                self.steps += inner.steps;
                branches.push(query);
            }
            self.at += end + 1;
            return Ok(Step::Union(branches));
        }
        Err(ParseError::NoHandler { at, rest: inside })
    }

    fn expression(&mut self, depth: usize) -> Result<Expression, ParseError> {
        if depth > MAX_DEPTH {
            return Err(ParseError::TooDeep {
                limit: MAX_DEPTH,
                at: self.at,
            });
        }
        let left = self.conjunction(depth)?;
        // 20 logical or
        self.skip_spaces();
        if self.eat("||") {
            let right = self.expression(depth + 1)?;
            return Ok(Expression::Or(Box::new(left), Box::new(right)));
        }
        Ok(left)
    }

    /// Needs no depth check of its own: it calls [`Parser::comparison`] before
    /// it recurses, and that refuses past [`MAX_DEPTH`], so a chain of `&&`
    /// never puts more than the bound on the stack.
    fn conjunction(&mut self, depth: usize) -> Result<Expression, ParseError> {
        let left = self.comparison(depth)?;
        // 19 logical and
        self.skip_spaces();
        if self.eat("&&") {
            let right = self.conjunction(depth + 1)?;
            return Ok(Expression::And(Box::new(left), Box::new(right)));
        }
        Ok(left)
    }

    fn comparison(&mut self, depth: usize) -> Result<Expression, ParseError> {
        if depth > MAX_DEPTH {
            return Err(ParseError::TooDeep {
                limit: MAX_DEPTH,
                at: self.at,
            });
        }
        self.skip_spaces();
        let at = self.at;
        // 21 logical not, reached only after the not-equal handler has had its
        // turn at this position: `!=` is not a negation of nothing.
        if self.rest().starts_with('!') && !self.rest().starts_with("!=") {
            self.at += 1;
            let inner = self.comparison(depth + 1)?;
            return Ok(Expression::Not(Box::new(inner)));
        }
        if self.eat("(") {
            let inner = self.expression(depth + 1)?;
            self.skip_spaces();
            if !self.eat(")") {
                return Err(ParseError::Unclosed {
                    opened: "grouping",
                    at,
                });
            }
            return Ok(inner);
        }
        let left = self.query(depth + 1)?;
        self.skip_spaces();
        let operator = self.operator()?;
        self.skip_spaces();
        let literal = self.literal()?;
        Ok(match operator {
            Operator::Less => Expression::Less(left, literal),
            Operator::LessOrEqual => Expression::LessOrEqual(left, literal),
            Operator::Greater => Expression::Greater(left, literal),
            Operator::GreaterOrEqual => Expression::GreaterOrEqual(left, literal),
            Operator::NotEqual => Expression::NotEqual(left, literal),
            Operator::Equal => Expression::Equal(left, literal),
            Operator::StringEqual => Expression::StringEqual(left, literal),
        })
    }

    /// Matches whole comparison tokens without changing their precedence.
    fn operator(&mut self) -> Result<Operator, ParseError> {
        // The published order is less-than before less-than-or-equal, and
        // first-match wins. Taken literally that leaves `<=` unreachable, since
        // `<` matches its first character; each operator is therefore matched
        // whole while the order is kept, which is the one place this grammar
        // reads the rule rather than the letter.
        let operator = if self.rest().starts_with("<=") {
            self.at += 2;
            Operator::LessOrEqual
        } else if self.rest().starts_with('<') {
            self.at += 1;
            Operator::Less
        } else if self.rest().starts_with(">=") {
            self.at += 2;
            Operator::GreaterOrEqual
        } else if self.rest().starts_with('>') {
            self.at += 1;
            Operator::Greater
        } else if self.rest().starts_with("!=") {
            self.at += 2;
            Operator::NotEqual
        } else if self.rest().starts_with("==") {
            self.at += 2;
            Operator::Equal
        } else if self.rest().starts_with("eq") {
            self.at += 2;
            Operator::StringEqual
        } else {
            return Err(ParseError::NoHandler {
                at: self.at,
                rest: self.rest().chars().take(16).collect(),
            });
        };
        Ok(operator)
    }

    fn literal(&mut self) -> Result<Literal, ParseError> {
        let at = self.at;
        if self.rest().starts_with('\'') {
            self.at += 1;
            let Some(end) = self.rest().find('\'') else {
                return Err(ParseError::Unclosed { opened: "text", at });
            };
            let text = self.rest()[..end].to_owned();
            self.at += end + 1;
            return Ok(Literal::Text(text));
        }
        if self.eat("true") {
            return Ok(Literal::Truth(true));
        }
        if self.eat("false") {
            return Ok(Literal::Truth(false));
        }
        let end = self
            .rest()
            .find(|character: char| {
                !character.is_ascii_digit() && character != '-' && character != '.'
            })
            .unwrap_or_else(|| self.rest().len());
        let written = &self.rest()[..end];
        match written.parse::<f64>() {
            Ok(number) if number.is_finite() => {
                self.at += end;
                Ok(Literal::Number(number))
            }
            _ => Err(ParseError::NoHandler {
                at,
                rest: written.to_owned(),
            }),
        }
    }

    fn name(&mut self) -> String {
        let end = self
            .rest()
            .find(|character: char| !character.is_ascii_alphanumeric() && character != '_')
            .unwrap_or_else(|| self.rest().len());
        let name = self.rest()[..end].to_owned();
        self.at += end;
        name
    }

    fn skip_spaces(&mut self) {
        while self.rest().starts_with(' ') {
            self.at += 1;
        }
    }
}

enum Operator {
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
    NotEqual,
    Equal,
    StringEqual,
}

#[cfg(test)]
mod tests {
    use super::{Expression, Literal, MAX_STEPS, ParseError, Step, parse};

    #[test]
    fn conjunction_binds_inside_disjunction_and_negation_wraps_the_comparison() {
        let query = parse("?(@.a == true || @.b <= 2 && !@.c eq '水')")
            .expect("logical operators and a Unicode text literal");
        let [Step::Filter(expression)] = query.steps() else {
            panic!("the filter must remain a single step");
        };
        let Expression::Or(left, right) = expression.as_ref() else {
            panic!("disjunction is the outer expression");
        };
        assert!(matches!(
            left.as_ref(),
            Expression::Equal(_, Literal::Truth(true))
        ));
        let Expression::And(left, right) = right.as_ref() else {
            panic!("conjunction binds more tightly than disjunction");
        };
        let Expression::LessOrEqual(_, literal) = left.as_ref() else {
            panic!("less-or-equal is a whole comparison token");
        };
        assert_eq!(literal, &Literal::Number(2.0));
        let Expression::Not(inner) = right.as_ref() else {
            panic!("negation wraps the complete comparison");
        };
        assert!(matches!(
            inner.as_ref(),
            Expression::StringEqual(_, Literal::Text(text)) if text == "水"
        ));
    }

    #[test]
    fn union_branches_share_the_step_budget_with_the_enclosing_query() {
        let first_steps = 1;
        let second_steps = MAX_STEPS - first_steps - 1;
        let first = ".a".repeat(first_steps);
        let second = ".a".repeat(second_steps);
        let at_limit = format!("[{first}|{second}]");
        let query = parse(&at_limit).expect("both branches and the union fit exactly");
        let [Step::Union(branches)] = query.steps() else {
            panic!("the two branches form one union");
        };
        assert_eq!(branches.len(), 2);
        assert_eq!(branches[0].steps().len(), first_steps);
        assert_eq!(branches[1].steps().len(), second_steps);
        assert_eq!(
            parse(&(at_limit + ".a")),
            Err(ParseError::TooManySteps { limit: MAX_STEPS })
        );
    }
}

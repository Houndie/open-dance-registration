const MIN_PASSWORD_LENGTH: usize = 8;

#[derive(Debug, Default, PartialEq)]
pub struct Validation {
    has_uppercase: bool,
    has_lowercase: bool,
    has_number: bool,
    has_special: bool,
    is_long_enough: bool,
}

impl Validation {
    pub fn new(password: &str) -> Validation {
        let mut validation = Validation::default();

        for c in password.chars() {
            if c.is_uppercase() {
                validation.has_uppercase = true;
            } else if c.is_lowercase() {
                validation.has_lowercase = true;
            } else if c.is_numeric() {
                validation.has_number = true;
            } else if !c.is_alphanumeric() {
                validation.has_special = true;
            }

            if validation.has_uppercase
                && validation.has_lowercase
                && validation.has_number
                && validation.has_special
            {
                break;
            };
        }

        if password.len() >= MIN_PASSWORD_LENGTH {
            validation.is_long_enough = true;
        };

        validation
    }

    pub fn is_valid(&self) -> bool {
        self.has_uppercase
            && self.has_lowercase
            && self.has_number
            && self.has_special
            && self.is_long_enough
    }
}

#[cfg(test)]
mod tests {
    use super::Validation;
    use test_case::test_case;

    enum Test {
        Correct,
        NoUppercase,
        NoLowercase,
        NoNumber,
        NoSpecial,
        NotLongEnough,
    }

    #[test_case(Test::Correct ; "correct")]
    #[test_case(Test::NoUppercase ; "no uppercase")]
    #[test_case(Test::NoLowercase ; "no lowercase")]
    #[test_case(Test::NoNumber ; "no number")]
    #[test_case(Test::NoSpecial ; "no special")]
    #[test_case(Test::NotLongEnough ; "not long enough")]
    fn test(test: Test) {
        struct TestCase {
            password: &'static str,
            expected: Validation,
        }

        let tc = match test {
            Test::Correct => TestCase {
                password: "aaAA11!!",
                expected: Validation {
                    has_uppercase: true,
                    has_lowercase: true,
                    has_number: true,
                    has_special: true,
                    is_long_enough: true,
                },
            },
            Test::NoUppercase => TestCase {
                password: "aabb11!!",
                expected: Validation {
                    has_uppercase: false,
                    has_lowercase: true,
                    has_number: true,
                    has_special: true,
                    is_long_enough: true,
                },
            },
            Test::NoLowercase => TestCase {
                password: "AABB11!!",
                expected: Validation {
                    has_uppercase: true,
                    has_lowercase: false,
                    has_number: true,
                    has_special: true,
                    is_long_enough: true,
                },
            },
            Test::NoNumber => TestCase {
                password: "aaBB@@!!",
                expected: Validation {
                    has_uppercase: true,
                    has_lowercase: true,
                    has_number: false,
                    has_special: true,
                    is_long_enough: true,
                },
            },
            Test::NoSpecial => TestCase {
                password: "aaBB11cc",
                expected: Validation {
                    has_uppercase: true,
                    has_lowercase: true,
                    has_number: true,
                    has_special: false,
                    is_long_enough: true,
                },
            },
            Test::NotLongEnough => TestCase {
                password: "aaBB11!",
                expected: Validation {
                    has_uppercase: true,
                    has_lowercase: true,
                    has_number: true,
                    has_special: true,
                    is_long_enough: false,
                },
            },
        };

        let actual = Validation::new(tc.password);
        assert_eq!(actual, tc.expected);
    }
}

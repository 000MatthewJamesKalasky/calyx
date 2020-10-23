use super::{Port, RRC};
use std::ops::{BitAnd, BitOr, Not};
use std::{cmp::Ordering, rc::Rc};

/// An assignment guard which has pointers to the various ports from which it reads.
#[derive(Debug, Clone)]
pub enum Guard {
    Or(Vec<Guard>),
    And(Vec<Guard>),
    Eq(Box<Guard>, Box<Guard>),
    Neq(Box<Guard>, Box<Guard>),
    Gt(Box<Guard>, Box<Guard>),
    Lt(Box<Guard>, Box<Guard>),
    Geq(Box<Guard>, Box<Guard>),
    Leq(Box<Guard>, Box<Guard>),
    Not(Box<Guard>),
    Port(RRC<Port>),
    True,
}

/// Helper functions for the guard.
impl Guard {
    /// Mutates a guard by calling `f` on every leaf in the
    /// guard tree and replacing the leaf with the guard that `f`
    /// returns.
    pub fn for_each<F>(&mut self, f: &F)
    where
        F: Fn(&Port) -> Option<Guard>,
    {
        match self {
            Guard::And(ands) => {
                ands.iter_mut().for_each(|guard| guard.for_each(f))
            }
            Guard::Or(ors) => {
                ors.iter_mut().for_each(|guard| guard.for_each(f))
            }
            Guard::Eq(l, r)
            | Guard::Neq(l, r)
            | Guard::Gt(l, r)
            | Guard::Lt(l, r)
            | Guard::Geq(l, r)
            | Guard::Leq(l, r) => {
                l.for_each(f);
                r.for_each(f);
            }
            Guard::Not(inner) => {
                inner.for_each(f);
            }
            Guard::Port(port) => {
                let guard = f(&port.borrow())
                    .unwrap_or_else(|| Guard::Port(Rc::clone(port)));
                *self = guard;
            }
            Guard::True => {}
        }
    }

    /// Returns all the ports used by this guard.
    pub fn all_ports(&self) -> Vec<RRC<Port>> {
        match self {
            Guard::Port(a) => vec![Rc::clone(a)],
            Guard::Or(gs) | Guard::And(gs) => {
                gs.iter().map(|g| g.all_ports()).flatten().collect()
            }
            Guard::Eq(l, r)
            | Guard::Neq(l, r)
            | Guard::Gt(l, r)
            | Guard::Lt(l, r)
            | Guard::Leq(l, r)
            | Guard::Geq(l, r) => {
                let mut atoms = l.all_ports();
                atoms.append(&mut r.all_ports());
                atoms
            }
            Guard::Not(g) => g.all_ports(),
            Guard::True => vec![],
        }
    }

    /// Return the string corresponding to the guard operation.
    pub fn op_str(&self) -> String {
        match self {
            Guard::And(_) => "&".to_string(),
            Guard::Or(_) => "|".to_string(),
            Guard::Eq(_, _) => "==".to_string(),
            Guard::Neq(_, _) => "!=".to_string(),
            Guard::Gt(_, _) => ">".to_string(),
            Guard::Lt(_, _) => "<".to_string(),
            Guard::Geq(_, _) => ">=".to_string(),
            Guard::Leq(_, _) => "<=".to_string(),
            Guard::Not(_) => "!".to_string(),
            Guard::Port(_) | Guard::True => {
                panic!("No operator string for Guard::Port")
            }
        }
    }

    pub fn and_vec(mut guards: Vec<Guard>) -> Self {
        if guards.len() == 1 {
            return guards.remove(0);
        }

        // Flatten any nested `And` inside the atoms.
        let mut flat_atoms: Vec<Guard> = Vec::with_capacity(guards.len());
        for atom in guards {
            match atom {
                Guard::And(mut bs) => flat_atoms.append(&mut bs),
                _ => flat_atoms.push(atom),
            }
        }
        // Filter out true guards
        flat_atoms.retain(|guard| {
            if let Guard::Port(p) = guard {
                return !p.borrow().is_constant(1);
            }

            !matches!(guard, Guard::True)
        });
        Guard::And(flat_atoms)
    }

    pub fn and(self, rhs: Guard) -> Self {
        Guard::and_vec(vec![self, rhs])
    }

    pub fn or(self, other: Guard) -> Self {
        Guard::Or(vec![self, other])
    }

    pub fn eq(self, other: Guard) -> Self {
        Guard::Eq(Box::new(self), Box::new(other))
    }

    pub fn neq(self, other: Guard) -> Self {
        Guard::Neq(Box::new(self), Box::new(other))
    }

    pub fn le(self, other: Guard) -> Self {
        Guard::Leq(Box::new(self), Box::new(other))
    }

    pub fn lt(self, other: Guard) -> Self {
        Guard::Lt(Box::new(self), Box::new(other))
    }

    pub fn ge(self, other: Guard) -> Self {
        Guard::Geq(Box::new(self), Box::new(other))
    }

    pub fn gt(self, other: Guard) -> Self {
        Guard::Gt(Box::new(self), Box::new(other))
    }

    pub fn not(self) -> Self {
        match self {
            Guard::Eq(lhs, rhs) => Guard::Neq(lhs, rhs),
            Guard::Neq(lhs, rhs) => Guard::Eq(lhs, rhs),
            Guard::Gt(lhs, rhs) => Guard::Leq(lhs, rhs),
            Guard::Lt(lhs, rhs) => Guard::Geq(lhs, rhs),
            Guard::Geq(lhs, rhs) => Guard::Lt(lhs, rhs),
            Guard::Leq(lhs, rhs) => Guard::Gt(lhs, rhs),
            Guard::Not(expr) => *expr,
            _ => Guard::Not(Box::new(self)),
        }
    }
}

/// Construct guards from ports
impl From<RRC<Port>> for Guard {
    fn from(port: RRC<Port>) -> Self {
        Guard::Port(Rc::clone(&port))
    }
}

impl PartialEq for Guard {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Guard::Or(_), Guard::Or(_)) => true,
            (Guard::And(_), Guard::And(_)) => true,
            (Guard::Eq(_, _), Guard::Eq(_, _)) => true,
            (Guard::Neq(_, _), Guard::Neq(_, _)) => true,
            (Guard::Gt(_, _), Guard::Gt(_, _)) => true,
            (Guard::Lt(_, _), Guard::Lt(_, _)) => true,
            (Guard::Geq(_, _), Guard::Geq(_, _)) => true,
            (Guard::Leq(_, _), Guard::Leq(_, _)) => true,
            (Guard::Not(_), Guard::Not(_)) => true,
            // XXX(rachit): This doesn't make sense. How can two different
            // ports be the same?
            (Guard::Port(_), Guard::Port(_)) => true,
            (Guard::True, Guard::True) => true,
            _ => false,
        }
    }
}

impl Eq for Guard {}

/// Define order on guards
impl PartialOrd for Guard {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Guard {
    fn cmp(&self, other: &Self) -> Ordering {
        if self == other {
            Ordering::Equal
        } else {
            match (self, other) {
                (Guard::Or(_), _) => Ordering::Greater,
                (_, Guard::Or(_)) => Ordering::Less,
                (Guard::And(_), _) => Ordering::Greater,
                (_, Guard::And(_)) => Ordering::Less,
                (Guard::Leq(..), _) => Ordering::Greater,
                (_, Guard::Leq(..)) => Ordering::Less,
                (Guard::Geq(..), _) => Ordering::Greater,
                (_, Guard::Geq(..)) => Ordering::Less,
                (Guard::Lt(..), _) => Ordering::Greater,
                (_, Guard::Lt(..)) => Ordering::Less,
                (Guard::Gt(..), _) => Ordering::Greater,
                (_, Guard::Gt(..)) => Ordering::Less,
                (Guard::Eq(..), _) => Ordering::Greater,
                (_, Guard::Eq(..)) => Ordering::Less,
                (Guard::Neq(..), _) => Ordering::Greater,
                (_, Guard::Neq(..)) => Ordering::Less,
                (Guard::Not(..), _) => Ordering::Greater,
                (_, Guard::Not(..)) => Ordering::Less,
                (Guard::Port(..), _) => Ordering::Greater,
                (_, Guard::Port(..)) => Ordering::Less,
                (Guard::True, _) => Ordering::Greater,
            }
        }
    }
}

/////////////// Sugar for convience constructors /////////////

/// Construct a Guard::And:
/// ```
/// let and_guard = g1 & g2;
/// ```
impl BitAnd for Guard {
    type Output = Self;

    fn bitand(self, other: Self) -> Self::Output {
        self.and(other)
    }
}

/// Construct a Guard::Or:
/// ```
/// let or_guard = g1 | g2;
/// ```
impl BitOr for Guard {
    type Output = Self;

    fn bitor(self, other: Self) -> Self::Output {
        self.or(other)
    }
}

/// Construct a Guard::Or:
/// ```
/// let not_guard = !g1;
/// ```
impl Not for Guard {
    type Output = Self;

    fn not(self) -> Self {
        self.not()
    }
}

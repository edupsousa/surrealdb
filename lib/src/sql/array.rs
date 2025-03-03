use crate::ctx::Context;
use crate::dbs::Options;
use crate::dbs::Transaction;
use crate::err::Error;
use crate::sql::comment::mightbespace;
use crate::sql::common::commas;
use crate::sql::error::IResult;
use crate::sql::fmt::{pretty_indent, Fmt, Pretty};
use crate::sql::number::Number;
use crate::sql::operation::Operation;
use crate::sql::value::{value, Value};
use nom::character::complete::char;
use nom::combinator::opt;
use nom::multi::separated_list0;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt::{self, Display, Formatter, Write};
use std::ops;
use std::ops::Deref;
use std::ops::DerefMut;

pub(crate) const TOKEN: &str = "$surrealdb::private::sql::Array";

#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize, Hash)]
#[serde(rename = "$surrealdb::private::sql::Array")]
pub struct Array(pub Vec<Value>);

impl From<Value> for Array {
	fn from(v: Value) -> Self {
		vec![v].into()
	}
}

impl From<Vec<Value>> for Array {
	fn from(v: Vec<Value>) -> Self {
		Self(v)
	}
}

impl From<Vec<i32>> for Array {
	fn from(v: Vec<i32>) -> Self {
		Self(v.into_iter().map(Value::from).collect())
	}
}

impl From<Vec<&str>> for Array {
	fn from(v: Vec<&str>) -> Self {
		Self(v.into_iter().map(Value::from).collect())
	}
}

impl From<Vec<String>> for Array {
	fn from(v: Vec<String>) -> Self {
		Self(v.into_iter().map(Value::from).collect())
	}
}

impl From<Vec<Number>> for Array {
	fn from(v: Vec<Number>) -> Self {
		Self(v.into_iter().map(Value::from).collect())
	}
}

impl From<Vec<Operation>> for Array {
	fn from(v: Vec<Operation>) -> Self {
		Self(v.into_iter().map(Value::from).collect())
	}
}

impl From<Array> for Vec<Value> {
	fn from(s: Array) -> Self {
		s.0
	}
}

impl FromIterator<Value> for Array {
	fn from_iter<I: IntoIterator<Item = Value>>(iter: I) -> Self {
		Array(iter.into_iter().collect())
	}
}

impl Deref for Array {
	type Target = Vec<Value>;
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl DerefMut for Array {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl IntoIterator for Array {
	type Item = Value;
	type IntoIter = std::vec::IntoIter<Self::Item>;
	fn into_iter(self) -> Self::IntoIter {
		self.0.into_iter()
	}
}

impl Array {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn with_capacity(len: usize) -> Self {
		Self(Vec::with_capacity(len))
	}
}

impl Array {
	pub(crate) async fn compute(
		&self,
		ctx: &Context<'_>,
		opt: &Options,
		txn: &Transaction,
		doc: Option<&Value>,
	) -> Result<Value, Error> {
		let mut x = Self::with_capacity(self.len());
		for v in self.iter() {
			match v.compute(ctx, opt, txn, doc).await {
				Ok(v) => x.push(v),
				Err(e) => return Err(e),
			};
		}
		Ok(Value::Array(x))
	}
}

impl Display for Array {
	fn fmt(&self, f: &mut Formatter) -> fmt::Result {
		let mut f = Pretty::from(f);
		f.write_char('[')?;
		if !self.is_empty() {
			let indent = pretty_indent();
			write!(f, "{}", Fmt::pretty_comma_separated(self.as_slice()))?;
			drop(indent);
		}
		f.write_char(']')
	}
}

// ------------------------------

impl ops::Add<Value> for Array {
	type Output = Self;
	fn add(mut self, other: Value) -> Self {
		self.0.push(other);
		self
	}
}

impl ops::Add for Array {
	type Output = Self;
	fn add(mut self, mut other: Self) -> Self {
		self.0.append(&mut other.0);
		self
	}
}

// ------------------------------

impl ops::Sub<Value> for Array {
	type Output = Self;
	fn sub(mut self, other: Value) -> Self {
		if let Some(p) = self.0.iter().position(|x| *x == other) {
			self.0.remove(p);
		}
		self
	}
}

impl ops::Sub for Array {
	type Output = Self;
	fn sub(mut self, other: Self) -> Self {
		for v in other.0 {
			if let Some(p) = self.0.iter().position(|x| *x == v) {
				self.0.remove(p);
			}
		}
		self
	}
}

// ------------------------------

pub trait Abolish<T> {
	fn abolish<F>(&mut self, f: F)
	where
		F: FnMut(usize) -> bool;
}

impl<T> Abolish<T> for Vec<T> {
	fn abolish<F>(&mut self, mut f: F)
	where
		F: FnMut(usize) -> bool,
	{
		let mut i = 0;
		// FIXME: use drain_filter once stabilized (https://github.com/rust-lang/rust/issues/43244)
		// to avoid negation of the predicate return value.
		self.retain(|_| {
			let retain = !f(i);
			i += 1;
			retain
		});
	}
}

// ------------------------------

pub(crate) trait Combine<T> {
	fn combine(self, other: T) -> T;
}

impl Combine<Array> for Array {
	fn combine(self, other: Self) -> Array {
		let mut out = Self::with_capacity(self.len().saturating_mul(other.len()));
		for a in self.iter() {
			for b in other.iter() {
				out.push(vec![a.clone(), b.clone()].into());
			}
		}
		out
	}
}

// ------------------------------

pub(crate) trait Complement<T> {
	fn complement(self, other: T) -> T;
}

impl Complement<Array> for Array {
	fn complement(self, other: Self) -> Array {
		let mut out = Array::new();
		for v in self.into_iter() {
			if !other.contains(&v) {
				out.push(v)
			}
		}
		out
	}
}

// ------------------------------

pub(crate) trait Concat<T> {
	fn concat(self, other: T) -> T;
}

impl Concat<Array> for Array {
	fn concat(mut self, mut other: Array) -> Array {
		self.append(&mut other);
		self
	}
}

// ------------------------------

pub(crate) trait Difference<T> {
	fn difference(self, other: T) -> T;
}

impl Difference<Array> for Array {
	fn difference(self, mut other: Array) -> Array {
		let mut out = Array::new();
		for v in self.into_iter() {
			if let Some(pos) = other.iter().position(|w| v == *w) {
				other.remove(pos);
			} else {
				out.push(v);
			}
		}
		out.append(&mut other);
		out
	}
}

// ------------------------------

pub(crate) trait Flatten<T> {
	fn flatten(self) -> T;
}

impl Flatten<Array> for Array {
	fn flatten(self) -> Array {
		let mut out = Array::new();
		for v in self.into_iter() {
			match v {
				Value::Array(mut a) => out.append(&mut a),
				_ => out.push(v),
			}
		}
		out
	}
}

// ------------------------------

pub(crate) trait Intersect<T> {
	fn intersect(self, other: T) -> T;
}

impl Intersect<Self> for Array {
	fn intersect(self, mut other: Self) -> Self {
		let mut out = Self::new();
		for v in self.0.into_iter() {
			if let Some(pos) = other.iter().position(|w| v == *w) {
				other.remove(pos);
				out.push(v);
			}
		}
		out
	}
}

// ------------------------------

pub(crate) trait Union<T> {
	fn union(self, other: T) -> T;
}

impl Union<Self> for Array {
	fn union(mut self, mut other: Self) -> Array {
		self.append(&mut other);
		self.uniq()
	}
}

// ------------------------------

pub(crate) trait Uniq<T> {
	fn uniq(self) -> T;
}

impl Uniq<Array> for Array {
	fn uniq(mut self) -> Array {
		let mut set: HashSet<&Value> = HashSet::new();
		let mut to_remove: Vec<usize> = Vec::new();
		for (i, item) in self.iter().enumerate() {
			if !set.insert(item) {
				to_remove.push(i);
			}
		}
		for i in to_remove.iter().rev() {
			self.remove(*i);
		}
		self
	}
}

// ------------------------------

pub fn array(i: &str) -> IResult<&str, Array> {
	let (i, _) = char('[')(i)?;
	let (i, _) = mightbespace(i)?;
	let (i, v) = separated_list0(commas, value)(i)?;
	let (i, _) = mightbespace(i)?;
	let (i, _) = opt(char(','))(i)?;
	let (i, _) = mightbespace(i)?;
	let (i, _) = char(']')(i)?;
	Ok((i, Array(v)))
}

#[cfg(test)]
mod tests {

	use super::*;

	#[test]
	fn array_empty() {
		let sql = "[]";
		let res = array(sql);
		assert!(res.is_ok());
		let out = res.unwrap().1;
		assert_eq!("[]", format!("{}", out));
		assert_eq!(out.0.len(), 0);
	}

	#[test]
	fn array_normal() {
		let sql = "[1,2,3]";
		let res = array(sql);
		assert!(res.is_ok());
		let out = res.unwrap().1;
		assert_eq!("[1, 2, 3]", format!("{}", out));
		assert_eq!(out.0.len(), 3);
	}

	#[test]
	fn array_commas() {
		let sql = "[1,2,3,]";
		let res = array(sql);
		assert!(res.is_ok());
		let out = res.unwrap().1;
		assert_eq!("[1, 2, 3]", format!("{}", out));
		assert_eq!(out.0.len(), 3);
	}

	#[test]
	fn array_expression() {
		let sql = "[1,2,3+1]";
		let res = array(sql);
		assert!(res.is_ok());
		let out = res.unwrap().1;
		assert_eq!("[1, 2, 3 + 1]", format!("{}", out));
		assert_eq!(out.0.len(), 3);
	}

	#[test]
	fn array_fnc_uniq_normal() {
		let sql = "[1,2,1,3,3,4]";
		let res = array(sql);
		assert!(res.is_ok());
		let out = res.unwrap().1.uniq();
		assert_eq!("[1, 2, 3, 4]", format!("{}", out));
		assert_eq!(out.0.len(), 4);
	}
}

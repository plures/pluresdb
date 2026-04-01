//! Proc-macro crate for `pluresdb-procedures`.
//!
//! Currently provides the [`pred!`] macro for compile-time predicate construction.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, Expr, ExprBinary, ExprLit, ExprPath, ExprUnary, Lit, Result, UnOp,
};

// ---------------------------------------------------------------------------
// Internal predicate representation (mirrors ir::Predicate)
// ---------------------------------------------------------------------------

enum PredNode {
    Comparison {
        field: String,
        cmp: &'static str,
        value: ValueNode,
    },
    And(Vec<PredNode>),
    Or(Vec<PredNode>),
    Not(Box<PredNode>),
}

enum ValueNode {
    Str(String),
    Float(f64),
    Int(i64),
    Bool(bool),
    #[allow(dead_code)]
    Null,
}

// ---------------------------------------------------------------------------
// Parse helpers
// ---------------------------------------------------------------------------

/// Extract a dotted field path like `data.score` or `category` from an expression.
fn expr_to_field(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Path(ExprPath { path, .. }) => {
            let segs: Vec<_> = path.segments.iter().map(|s| s.ident.to_string()).collect();
            Some(segs.join("."))
        }
        Expr::Field(f) => {
            let base = expr_to_field(&f.base)?;
            let member = match &f.member {
                syn::Member::Named(n) => n.to_string(),
                syn::Member::Unnamed(idx) => idx.index.to_string(),
            };
            Some(format!("{}.{}", base, member))
        }
        _ => None,
    }
}

/// Extract a literal value from an expression.
fn expr_to_value(expr: &Expr) -> Option<ValueNode> {
    match expr {
        Expr::Lit(ExprLit { lit, .. }) => match lit {
            Lit::Str(s) => Some(ValueNode::Str(s.value())),
            Lit::Float(f) => f.base10_parse::<f64>().ok().map(ValueNode::Float),
            Lit::Int(i) => i.base10_parse::<i64>().ok().map(ValueNode::Int),
            Lit::Bool(b) => Some(ValueNode::Bool(b.value)),
            _ => None,
        },
        _ => None,
    }
}

/// Recursively build a `PredNode` from a Rust `Expr`.
fn expr_to_pred(expr: &Expr) -> std::result::Result<PredNode, syn::Error> {
    match expr {
        Expr::Binary(ExprBinary {
            left, op, right, ..
        }) => {
            use syn::BinOp;
            match op {
                BinOp::And(_) => {
                    let l = expr_to_pred(left)?;
                    let r = expr_to_pred(right)?;
                    Ok(PredNode::And(vec![l, r]))
                }
                BinOp::Or(_) => {
                    let l = expr_to_pred(left)?;
                    let r = expr_to_pred(right)?;
                    Ok(PredNode::Or(vec![l, r]))
                }
                _ => {
                    // Comparison
                    let cmp = match op {
                        BinOp::Eq(_) => "==",
                        BinOp::Ne(_) => "!=",
                        BinOp::Gt(_) => ">",
                        BinOp::Ge(_) => ">=",
                        BinOp::Lt(_) => "<",
                        BinOp::Le(_) => "<=",
                        other => {
                            return Err(syn::Error::new_spanned(
                                other,
                                "unsupported comparison operator; expected ==, !=, >, >=, <, <=",
                            ))
                        }
                    };
                    let field = expr_to_field(left).ok_or_else(|| {
                        syn::Error::new_spanned(
                            left.clone(),
                            "left-hand side of comparison must be a field path (e.g. `category` or `data.score`)",
                        )
                    })?;
                    let value = expr_to_value(right).ok_or_else(|| {
                        syn::Error::new_spanned(
                            right.clone(),
                            "right-hand side of comparison must be a string, number, or bool literal",
                        )
                    })?;
                    Ok(PredNode::Comparison { field, cmp, value })
                }
            }
        }
        Expr::Unary(ExprUnary { op: UnOp::Not(_), expr: inner, .. }) => {
            let p = expr_to_pred(inner)?;
            Ok(PredNode::Not(Box::new(p)))
        }
        Expr::Paren(paren) => expr_to_pred(&paren.expr),
        other => Err(syn::Error::new_spanned(
            other,
            "pred! expression must be a comparison (field op value) or a logical combination using && / || / !",
        )),
    }
}

// ---------------------------------------------------------------------------
// Code generation
// ---------------------------------------------------------------------------

fn pred_node_to_tokens(node: PredNode) -> TokenStream2 {
    let crate_path = quote! { ::pluresdb_procedures::ir };
    match node {
        PredNode::Comparison { field, cmp, value } => {
            let cmp_variant = match cmp {
                "==" => quote! { #crate_path::CmpOp::Eq },
                "!=" => quote! { #crate_path::CmpOp::Ne },
                ">" => quote! { #crate_path::CmpOp::Gt },
                ">=" => quote! { #crate_path::CmpOp::Ge },
                "<" => quote! { #crate_path::CmpOp::Lt },
                "<=" => quote! { #crate_path::CmpOp::Le },
                _ => unreachable!(),
            };
            let value_tokens = match value {
                ValueNode::Str(s) => quote! { #crate_path::IrValue::String(#s.to_string()) },
                ValueNode::Float(f) => quote! { #crate_path::IrValue::Number(#f) },
                ValueNode::Int(i) => quote! { #crate_path::IrValue::Number(#i as f64) },
                ValueNode::Bool(b) => quote! { #crate_path::IrValue::Bool(#b) },
                ValueNode::Null => quote! { #crate_path::IrValue::Null },
            };
            quote! {
                #crate_path::Predicate::Comparison {
                    field: #field.to_string(),
                    cmp: #cmp_variant,
                    value: #value_tokens,
                }
            }
        }
        PredNode::And(children) => {
            let children_tokens: Vec<_> = children.into_iter().map(pred_node_to_tokens).collect();
            quote! {
                #crate_path::Predicate::And {
                    and: vec![#(#children_tokens),*],
                }
            }
        }
        PredNode::Or(children) => {
            let children_tokens: Vec<_> = children.into_iter().map(pred_node_to_tokens).collect();
            quote! {
                #crate_path::Predicate::Or {
                    or: vec![#(#children_tokens),*],
                }
            }
        }
        PredNode::Not(inner) => {
            let inner_tokens = pred_node_to_tokens(*inner);
            quote! {
                #crate_path::Predicate::Not {
                    not: ::std::boxed::Box::new(#inner_tokens),
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Public macro
// ---------------------------------------------------------------------------

struct PredInput {
    expr: Expr,
}

impl Parse for PredInput {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(PredInput {
            expr: input.parse()?,
        })
    }
}

/// Construct a [`pluresdb_procedures::ir::Predicate`] at compile time.
///
/// # Syntax
///
/// ```rust,ignore
/// use pluresdb_procedures::pred;
///
/// let p = pred!(category == "decision");
/// let p = pred!(data.score > 0.7);
/// let p = pred!(category == "decision" && data.score > 0.7);
/// let p = pred!(status == "open" || status == "pending");
/// let p = pred!(!archived);  // not equal to true — not yet supported; use pred!(archived == false)
/// ```
///
/// # Compile-time errors
///
/// Any syntax error (wrong operator, non-literal value, etc.) is reported as a
/// compiler error at the call site:
///
/// ```rust,compile_fail
/// use pluresdb_procedures::pred;
/// let _p = pred!(123 == "oops");  // Error: LHS must be a field path
/// ```
#[proc_macro]
pub fn pred(input: TokenStream) -> TokenStream {
    let PredInput { expr } = parse_macro_input!(input as PredInput);
    let node = match expr_to_pred(&expr) {
        Ok(n) => n,
        Err(e) => return e.to_compile_error().into(),
    };
    pred_node_to_tokens(node).into()
}

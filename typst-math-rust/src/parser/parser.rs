//! Parser module, traverse the AST to generate decorations

use crate::interface::{Decoration, Options};
use std::collections::HashMap;
use typst_syntax::ast::{
    AstNode, FieldAccess, FuncCall, Linebreak, Math, MathAttach, MathIdent, Shorthand, Str, Text,
};
use typst_syntax::{ast::Expr, SyntaxNode};
use typst_syntax::{LinkedNode, SyntaxKind};

use crate::utils::symbols::{Color, BLACKBOLD_LETTERS, CAL_LETTERS, FRAK_LETTERS};

use super::utils::{get_symbol, insert_result, insert_result_symbol, insert_void};

/// State of the parser, used to know if we are in a base, attachment, or other
#[derive(Clone)]
pub struct State {
    pub is_base: bool,
    pub is_attachment: bool,
}

/// Inner code of the DFS, used to traverse the AST and apply style
/// Most complex part of the code, match the current expression and then,
/// compute the appropriate style and/or if we need to continue over children
pub fn inner_ast_dfs(
    source: &typst_syntax::Source,
    expr: Expr,
    result: &mut HashMap<String, Decoration>,
    state: State,
    uuid: &str,
    added_text_decoration: &str,
    offset: (usize, usize),
    options: &Options,
) {
    match expr {
        // Math identifier, check if it is in the symbols list
        Expr::MathIdent(ident) => math_ident_block(
            source,
            ident,
            result,
            state,
            uuid,
            added_text_decoration,
            offset,
            options,
        ),
        // Field Access, create a string containing all fields sparated with a dot (alpha.alt), and check if it is in symbols list
        Expr::FieldAccess(access) => field_access_block(
            source,
            access,
            result,
            state,
            uuid,
            added_text_decoration,
            offset,
            options,
        ),
        // Replace linebreak with an arrow
        Expr::Linebreak(lbreak) => linebreak_block(
            source,
            lbreak,
            result,
            state,
            uuid,
            added_text_decoration,
            offset,
            options,
        ),
        // Math attachment, power, subscript, superscript
        Expr::MathAttach(attachment) => {
            math_attach_block(
                source,
                attachment,
                result,
                state,
                uuid,
                added_text_decoration,
                offset,
                options,
            );
        }
        // Math block, continue over children and check current state to apply style
        Expr::Math(math) => {
            math_block(
                source,
                math,
                result,
                state,
                uuid,
                added_text_decoration,
                offset,
                options,
            );
        }
        // Typst shorthands
        Expr::Shorthand(short) => {
            shorthand_block(
                source,
                short,
                result,
                state,
                uuid,
                added_text_decoration,
                offset,
                options,
            );
        }
        // Typst text block, some symbols are here instead of shorthand
        Expr::Text(text) => {
            text_block(
                source,
                text,
                result,
                state,
                uuid,
                added_text_decoration,
                offset,
                options,
            );
        }
        // Typst string block (between quotes)
        Expr::Str(text) => {
            str_block(
                source,
                text,
                result,
                state,
                uuid,
                added_text_decoration,
                offset,
                options,
            );
        }
        // Typst func, if it's a common func, apply style, else continue over args and callee
        Expr::FuncCall(func) => {
            func_call_block(
                source,
                func,
                result,
                state,
                uuid,
                added_text_decoration,
                offset,
                options,
            );
        }
        _ => {
            if let Some(child) = source.find(expr.span()) {
                ast_dfs(
                    source,
                    &child,
                    result,
                    state,
                    uuid,
                    added_text_decoration,
                    options,
                ); // Propagate the function
            }
        }
    }
}

/// Use a recursive DFS to traverse the entire AST
pub fn ast_dfs(
    source: &typst_syntax::Source,
    node: &SyntaxNode,
    result: &mut HashMap<String, Decoration>,
    state: State,
    uuid: &str,
    added_text_decoration: &str,
    options: &Options,
) {
    for child in node.children() {
        if let Some(expr) = child.cast::<Expr>() {
            inner_ast_dfs(
                source,
                expr,
                result,
                state.clone(),
                uuid,
                added_text_decoration,
                (0, 0),
                options,
            )
        } else {
            ast_dfs(
                source,
                child,
                result,
                state.clone(),
                uuid,
                added_text_decoration,
                options,
            );
        }
    }
}

/// Recursive function to convert a field access into a string (`[alpha, ., alt]` -> `'alpha.alt'`)
fn field_access_recursive(access: FieldAccess) -> Option<String> {
    // Check if the target is a math identifier or another field access
    match access.target() {
        Expr::FieldAccess(subaccess) => {
            if let Some(start) = field_access_recursive(subaccess) {
                return Some(format!("{}.{}", start, access.field().to_string()));
            }
        }
        Expr::MathIdent(ident) => {
            return Some(format!(
                "{}.{}",
                ident.to_string(),
                access.field().to_string()
            ));
        }
        Expr::Ident(ident) => {
            return Some(format!(
                "{}.{}",
                ident.to_string(),
                access.field().to_string()
            ));
        }
        _ => {}
    }
    None
}

fn math_ident_block(
    source: &typst_syntax::Source,
    ident: MathIdent,
    result: &mut HashMap<String, Decoration>,
    _state: State,
    uuid: &str,
    added_text_decoration: &str,
    offset: (usize, usize),
    options: &Options,
) {
    insert_result_symbol(
        source,
        ident.span(),
        ident.to_string(),
        format!("{uuid}-{}", ident.to_string()),
        result,
        added_text_decoration,
        offset,
        ("", ""),
        options,
    );
}
fn field_access_block(
    source: &typst_syntax::Source,
    access: FieldAccess,
    result: &mut HashMap<String, Decoration>,
    _state: State,
    uuid: &str,
    added_text_decoration: &str,
    offset: (usize, usize),
    options: &Options,
) {
    if let Some(content) = field_access_recursive(access) {
        let mut offset = offset;
        // Add one to offset to remove the # with sym
        if content.contains("sym") {
            if options.render_outside_math {
                offset.0 = offset.0 + 1;
            } else {
                return;
            }
        }

        let content = content.replace("sym.", "");
        insert_result_symbol(
            source,
            access.span(),
            content.clone(),
            format!("{uuid}-{}", content),
            result,
            added_text_decoration,
            offset,
            ("", ""),
            options,
        );
    }
}
fn linebreak_block(
    source: &typst_syntax::Source,
    lbreak: Linebreak,
    result: &mut HashMap<String, Decoration>,
    _state: State,
    uuid: &str,
    added_text_decoration: &str,
    offset: (usize, usize),
    options: &Options,
) {
    insert_result(
        source,
        lbreak.span(),
        format!("{uuid}-linebreak"),
        '⮰'.to_string(),
        Color::Comparison,
        format!(
            "{}font-family: NewComputerModernMath; font-weight: bold;",
            added_text_decoration
        ),
        result,
        offset,
        options,
    );
}

fn math_attach_block(
    source: &typst_syntax::Source,
    attachment: MathAttach,
    result: &mut HashMap<String, Decoration>,
    _state: State,
    uuid: &str,
    added_text_decoration: &str,
    offset: (usize, usize),
    options: &Options,
) {
    if let Some(child) = source.find(attachment.span()) {
        // Check if it is the 'main' base, and render it if true
        if child.parent_kind() != Some(SyntaxKind::MathAttach) {
            inner_ast_dfs(
                source,
                attachment.base(),
                result,
                State {
                    is_base: true,
                    is_attachment: false,
                },
                uuid,
                added_text_decoration,
                offset,
                options,
            );
        } else {
            inner_ast_dfs(
                source,
                attachment.base(),
                result,
                State {
                    is_base: false,
                    is_attachment: false,
                },
                "",
                "",
                (0, 0),
                options,
            );
        }
    }
    // Compute specific offset and style with rendering mode
    let offset = if options.rendering_mode > 1 {
        (1, 0)
    } else {
        offset
    };
    let top_decor = if options.rendering_mode > 1 {
        "font-size: 0.8em; transform: translateY(-30%); display: inline-block;"
    } else {
        ""
    };
    let bottom_decor = if options.rendering_mode > 1 {
        "font-size: 0.8em; transform: translateY(20%); display: inline-block;"
    } else {
        ""
    };
    if let Some(top) = attachment.top() {
        inner_ast_dfs(
            source,
            top,
            result,
            State {
                is_base: false,
                is_attachment: options.rendering_mode > 1,
            },
            "top-",
            top_decor,
            offset,
            options,
        )
    }
    if let Some(bottom) = attachment.bottom() {
        inner_ast_dfs(
            source,
            bottom,
            result,
            State {
                is_base: false,
                is_attachment: options.rendering_mode > 1,
            },
            "bottom-",
            bottom_decor,
            offset,
            options,
        )
    }
}

fn math_block(
    source: &typst_syntax::Source,
    math: Math,
    result: &mut HashMap<String, Decoration>,
    state: State,
    uuid: &str,
    added_text_decoration: &str,
    offset: (usize, usize),
    options: &Options,
) {
    if let Some(child) = source.find(math.span()) {
        let children: Vec<LinkedNode> = child.children().collect();
        // If we are in an attachment, check if the current math block is just paren around a symbol
        if children.len() == 3
            && children[0].kind() == SyntaxKind::LeftParen
            && children[1].kind() == SyntaxKind::Math
            && children[2].kind() == SyntaxKind::RightParen
        {
            // This serie of check aims to verify that the block inside paren is 'simple', wich means that we can propagate style (So top and bottom attachment)
            let mut propagate_style = false;
            let sub_children: Vec<LinkedNode> = children[1].children().collect();

            // Check if it's just a text
            if sub_children.len() == 1
                && (sub_children[0].kind() == SyntaxKind::Text
                    || sub_children[0].kind() == SyntaxKind::Str)
            {
                propagate_style = true;
            }
            // Check if it's just a symbol
            else if sub_children.len() == 1 && sub_children[0].kind() == SyntaxKind::MathIdent {
                if get_symbol(
                    sub_children[0].cast::<MathIdent>().unwrap().to_string(),
                    options,
                )
                .is_some()
                {
                    propagate_style = true;
                }
            }
            // Check if it's a text with a sign
            else if sub_children.len() == 2
                && sub_children[0].kind() == SyntaxKind::Shorthand
                && (sub_children[1].kind() == SyntaxKind::Text
                    || sub_children[1].kind() == SyntaxKind::Str)
            {
                propagate_style = true;
            }
            // Check if it's a symbol with a sign
            else if sub_children.len() == 2
                && sub_children[0].kind() == SyntaxKind::Shorthand
                && sub_children[1].kind() == SyntaxKind::MathIdent
            {
                if get_symbol(
                    sub_children[1].cast::<MathIdent>().unwrap().to_string(),
                    options,
                )
                .is_some()
                {
                    propagate_style = true;
                }
            }

            // We can propagate, hide paren and then continue over children (With a for loop and a call to inner, to keep current style)
            if propagate_style {
                insert_void(source, children[0].span(), result, (offset.0, 0), options);
                insert_void(source, children[2].span(), result, (0, offset.1), options);
                for child in children[1].children() {
                    if let Some(expr) = child.cast::<Expr>() {
                        inner_ast_dfs(
                            source,
                            expr,
                            result,
                            state.clone(),
                            uuid,
                            added_text_decoration,
                            (0, 0),
                            options,
                        );
                    }
                }
                return;
            }
        }
        ast_dfs(source, &child, result, state, "", "", options); // Propagate the function
    }
}

fn shorthand_block(
    source: &typst_syntax::Source,
    short: Shorthand,
    result: &mut HashMap<String, Decoration>,
    _state: State,
    uuid: &str,
    added_text_decoration: &str,
    offset: (usize, usize),
    options: &Options,
) {
    let (color, decoration, content) = match short.get() {
        // Apply specific style for each shorthand
        '\u{2212}' => (Color::Operator, "", '-'),
        '∗' => (Color::Operator, "", '*'),
        '⟦' | '⟧' => (Color::Set, "", short.get()),
        c => (
            Color::Comparison,
            "font-family: \"NewComputerModernMath\"; font-weight: bold;",
            c,
        ),
    };
    insert_result(
        source,
        short.span(),
        format!("{uuid}-{}", content.to_string()),
        content.to_string(),
        color,
        format!("{}{}", added_text_decoration, decoration),
        result,
        offset,
        options,
    );
}
fn text_block(
    source: &typst_syntax::Source,
    text: Text,
    result: &mut HashMap<String, Decoration>,
    state: State,
    uuid: &str,
    added_text_decoration: &str,
    offset: (usize, usize),
    options: &Options,
) {
    if text.get().len() == 1 {
        if let Some((color, decoration)) = match text.get().as_str() {
            "+" => Some((Color::Operator, "")),
            "=" | "<" | ">" => Some((Color::Comparison, "")),
            "[" | "]" => Some((Color::Set, "")),
            _ => None,
        } {
            insert_result(
                source,
                text.span(),
                format!("{uuid}-{}", text.get().to_string()),
                text.get().to_string(),
                color,
                format!("{}{}", added_text_decoration, decoration),
                result,
                offset,
                options,
            );
            return;
        }
    }
    if state.is_attachment {
        insert_result(
            source,
            text.span(),
            format!("{uuid}-text-{}", text.get().to_string()),
            text.get().to_string(),
            Color::Number,
            format!("{}", added_text_decoration),
            result,
            offset,
            options,
        );
    }
}
fn str_block(
    source: &typst_syntax::Source,
    text: Str,
    result: &mut HashMap<String, Decoration>,
    state: State,
    uuid: &str,
    added_text_decoration: &str,
    offset: (usize, usize),
    options: &Options,
) {
    if state.is_attachment {
        insert_result(
            source,
            text.span(),
            format!("{uuid}-text-{}", text.get().to_string()),
            text.get().to_string(),
            Color::Number,
            format!("{}", added_text_decoration),
            result,
            offset,
            options,
        );
    }
}
fn func_call_block(
    source: &typst_syntax::Source,
    func: FuncCall,
    result: &mut HashMap<String, Decoration>,
    state: State,
    uuid: &str,
    added_text_decoration: &str,
    offset: (usize, usize),
    options: &Options,
) {
    let args = func.args().to_untyped();
    let children: Vec<&SyntaxNode> = args.children().collect();
    let mut propagate_style = true;

    // If there is just a text, try to apply a text func like blackbold, caligraphy...
    if args.children().len() == 3
        && children[0].kind() == SyntaxKind::LeftParen
        && (children[1].kind() == SyntaxKind::Text || children[1].kind() == SyntaxKind::Str)
        && children[2].kind() == SyntaxKind::RightParen
        && options.rendering_mode > 1
    {
        let text = children[1];
        let text_content = match text.kind() {
            SyntaxKind::Text => text.cast::<Text>().unwrap().get().to_string(),
            SyntaxKind::Str => text.cast::<Str>().unwrap().get().to_string(),
            _ => "".to_string(),
        };
        match func.callee() {
            Expr::MathIdent(ident) => {
                if let Some((map, decoration)) = match ident.as_str() {
                    "cal" => Some((CAL_LETTERS, "font-family: \"NewComputerModernMath\";")),
                    "frak" => Some((FRAK_LETTERS, "font-family: \"NewComputerModernMath\";")),
                    "bb" => Some((BLACKBOLD_LETTERS, "")),
                    _ => None,
                } {
                    let mut symbol = String::new();
                    for letter in text_content.chars() {
                        if let Some(c) = map.get(&letter) {
                            symbol.push(*c);
                        } else {
                            symbol.push(letter);
                        }
                    }
                    insert_result(
                        source,
                        text.span(),
                        format!("{uuid}-{}", symbol),
                        symbol,
                        Color::Number,
                        format!("{}{}", added_text_decoration, decoration),
                        result,
                        (ident.as_str().len() + 1 + offset.0, 1 + offset.1),
                        options,
                    );
                    return;
                }
            }
            _ => {}
        }
    }
    if options.rendering_mode > 2 {
        if let Some((span, content)) = match func.callee() {
            Expr::MathIdent(ident) => Some((ident.span(), ident.to_string())),
            Expr::FieldAccess(access) => {
                if let Some(content) = field_access_recursive(access) {
                    Some((access.span(), content))
                } else {
                    None
                }
            }
            _ => None,
        } {
            if let Some((symbol, decoration)) = match content.as_str() {
            "arrow" => Some((
                '→',
                "font-family: \"NewComputerModernMath\"; transform: translate(-0.1em, -0.9em); font-size: 0.8em; display: inline-block; position: absolute;",
            )),
            "dot" => Some((
                '⋅',
                "font-family: \"Fira Math\";
                transform: translate(0.15em, -0.55em);
                transform: translate(0.15em, -0.52em); display: inline-block; position: absolute;",
            )),
            "dot.double" | "diaer" => Some(('¨', "font-family: JuliaMono; transform: translate(0, -0.25em); display: inline-block; position: absolute;")),
            "dot.triple" => Some(('\u{20DB}', "font-family: JuliaMono; font-size: 1.4em; transform: translate(-0.1em); display: inline-block;")),
            "dot.quad" => Some(('\u{20DC}', "font-family: JuliaMono; font-size: 1.4em; transform: translate(-0.1em); display: inline-block;")),
            "hat" => Some((
                '^',
                "font-family: Fira math; transform: translate(0.03em, -0.3em); font-size: 0.9em; display: inline-block; position: absolute;",
            )),
            "tilde" => Some((
                '~',
                "font-family: JuliaMono; transform: translate(0.05em, -0.7em); font-size: 0.9em; display: inline-block; position: absolute;",
            )),
            "overline" => Some(('\u{0305}', "font-family: JuliaMono; transform: translate(0em, -0.2em); display: inline-block;")),
            _ => None,
        } {
            if args.children().len() == 3
                && children[0].kind() == SyntaxKind::LeftParen
                && (children[1].kind() == SyntaxKind::MathIdent
                    || children[1].kind() == SyntaxKind::Text
                    || (children[1].kind() == SyntaxKind::MathAttach && children[1].children().len() == 3))
                && children[2].kind() == SyntaxKind::RightParen
            {
                insert_result(
                    source,
                    span,
                    format!("{uuid}-func-{}", symbol),
                    symbol.to_string(),
                    Color::Number,
                    format!("{}", decoration),
                    result,
                    (0, 1),
                    options,
                );
                insert_void(source, children[2].span(), result, (0, 0), options);
                propagate_style = false;
            }
        } else if let Some(symbol) = match content.as_str() {
            "abs" => Some('|'),
            "norm" => Some('‖'),
            _ => None,
        } {
            insert_void(source, span, result, (offset.0, 0), options);
            insert_result(
                source,
                children[0].span(),
                format!("{uuid}-func-{}", symbol),
                symbol.to_string(),
                Color::Operator,
                format!("{}", added_text_decoration),
                result,
                (0,0),
                options,
            );
            insert_result(
                source,
                children.last().unwrap().span(),
                format!("{uuid}-func-{}", symbol),
                symbol.to_string(),
                Color::Operator,
                format!("{}", added_text_decoration),
                result,
                (0, offset.1),
                options,
            );
        } else if content.as_str() == "sqrt" && args.children().len() == 3 && children[0].kind() == SyntaxKind::LeftParen && children[2].kind() == SyntaxKind::RightParen {
            let mut root_size = None;
            if children[1].kind() == SyntaxKind::MathIdent || children[1].kind() == SyntaxKind::Text {
                root_size = Some(1.2);
            } else if children[1].kind() == SyntaxKind::MathAttach && children[1].children().len() == 3 &&
                (children[1].children().nth(2).unwrap().kind() == SyntaxKind::MathIdent
                || children[1].children().nth(2).unwrap().kind() == SyntaxKind::Text
            ) {
                root_size = Some(1.8);
            }
            if root_size.is_some() {
                insert_result(
                    source,
                    children[0].span(),
                    format!("{uuid}-func-{}-size-{}", '\u{0305}', root_size.unwrap()),
                    '\u{0305}'.to_string(),
                    Color::Operator,
                    format!(
                        "font-family: JuliaMono; transform: scaleX({:.1}) translate(-0.01em, -0.25em); display: inline-block;",
                        root_size.unwrap()
                    ),
                    result,
                    (0, 0),
                    options,
                );
                insert_result(
                    source,
                    span,
                    format!("{uuid}-func-{}", '√'),
                    '√'.to_string(),
                    Color::Operator,
                    format!("font-family: JuliaMono; display: inline-block; transform: translate(0.1em, -0.1em);"),
                    result,
                    (0, 0),
                    options,
                );
                insert_void(source, children[2].span(), result, (0, 0), options);
                propagate_style = false;
            }
        } else {
            inner_ast_dfs(source, func.callee(), result, state.clone(), uuid, added_text_decoration, offset, options);
            propagate_style = false;
        }
        }
    } else {
        propagate_style = false;
    }
    ast_dfs(
        source,
        func.args().to_untyped(),
        result,
        state,
        if propagate_style { uuid } else { "" },
        if propagate_style {
            added_text_decoration
        } else {
            ""
        },
        options,
    );
}

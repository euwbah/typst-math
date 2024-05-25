//! Parser module, traverse the AST to generate decorations

use super::utils::{get_symbol, unchecked_cast_expr, InnerParser};
use crate::utils::symbols::{Color, BLACKBOLD_LETTERS, CAL_LETTERS, FRAK_LETTERS};
use typst_syntax::ast::{
    AstNode, Expr, FieldAccess, FuncCall, Linebreak, MathAttach, MathIdent, Shorthand, Str, Text,
};
use typst_syntax::{SyntaxKind, SyntaxNode};

/// State of the parser, used to know if we are in a base, attachment, or other
#[derive(Clone)]
pub struct State {
    pub is_base: bool,
    pub is_attachment: bool,
}

/// Inner code of the DFS, used to traverse the AST and apply style \
/// Most complex part of the code, match the current expression and then,
/// compute the appropriate style and/or if we need to continue over children
pub fn inner_ast_dfs(
    parser: &mut InnerParser,
    expr: Expr,
    uuid: &str,
    added_text_decoration: &str,
    offset: (usize, usize),
) {
    // Create the new parser
    let mut parser = InnerParser::from(
        parser,
        expr.to_untyped(),
        uuid,
        added_text_decoration,
        offset,
    );
    // Math the current expression type
    if let Some(_) = match expr {
        // Math identifier, check if it is in the symbols list
        Expr::MathIdent(_) => Some(math_ident_block(&mut parser)),
        // Field Access, create a string containing all fields sparated with a dot (alpha.alt), and check if it is in symbols list
        Expr::FieldAccess(_) => Some(field_access_block(&mut parser)),
        // Replace linebreak with an arrow
        Expr::Linebreak(_) => Some(linebreak_block(&mut parser)),
        // Math attachment, power, subscript, superscript
        Expr::MathAttach(_) => Some(math_attach_block(&mut parser)),
        // Math block, continue over children and check current state to apply style
        Expr::Math(_) => Some(math_block(&mut parser)),
        // Typst shorthands
        Expr::Shorthand(_) => Some(shorthand_block(&mut parser)),
        // Typst text block, some symbols are here instead of shorthand
        Expr::Text(_) => Some(text_block(&mut parser)),
        // Typst string block (between quotes)
        Expr::Str(_) => Some(str_block(&mut parser)),
        // Typst func, if it's a common func, apply style, else continue over args and callee
        Expr::FuncCall(_) => Some(func_call_block(&mut parser)),
        _ => None,
    } {
    } else {
        // If there is no math, just iterate over children
        let expr = parser.expr;
        ast_dfs(&mut parser, expr, uuid, added_text_decoration); // Propagate the function
    }
}

/// Use a recursive DFS to traverse the entire AST
pub fn ast_dfs(
    parser: &mut InnerParser,
    node: &SyntaxNode,
    uuid: &str,
    added_text_decoration: &str,
) {
    for child in node.children() {
        if let Some(expr) = child.cast::<Expr>() {
            inner_ast_dfs(parser, expr, uuid, added_text_decoration, (0, 0))
        } else {
            ast_dfs(parser, child, uuid, added_text_decoration);
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

fn math_ident_block(parser: &mut InnerParser) {
    let ident = unchecked_cast_expr::<MathIdent>(parser.expr);
    parser.insert_result_symbol(
        ident.span(),
        ident.to_string(),
        format!("{}-{}", parser.uuid, ident.to_string()),
        parser.added_text_decoration,
        parser.offset,
        ("", ""),
    );
}
fn field_access_block(parser: &mut InnerParser) {
    let access = unchecked_cast_expr::<FieldAccess>(parser.expr);
    if let Some(content) = field_access_recursive(access) {
        // Add one to offset to remove the # with sym
        if content.contains("sym") {
            if parser.options.render_outside_math {
                parser.offset.0 += 1;
            } else {
                return;
            }
        }

        let content = content.replace("sym.", "");
        parser.insert_result_symbol(
            access.span(),
            content.clone(),
            format!("{}-{}", parser.uuid, content),
            parser.added_text_decoration,
            parser.offset,
            ("", ""),
        );
    }
}
fn linebreak_block(parser: &mut InnerParser) {
    let lbreak = unchecked_cast_expr::<Linebreak>(parser.expr);
    parser.insert_result(
        lbreak.span(),
        format!("{}-linebreak", parser.uuid),
        '⮰'.to_string(),
        Color::Comparison,
        format!(
            "{}font-family: NewComputerModernMath; font-weight: bold;",
            parser.added_text_decoration
        ),
        parser.offset,
    );
}

fn math_attach_block(parser: &mut InnerParser) {
    let attachment = unchecked_cast_expr::<MathAttach>(parser.expr);
    // Keep the current state to restore it after the attachment
    let state = State {
        is_base: parser.state.is_base,
        is_attachment: parser.state.is_attachment,
    };
    if let Some(child) = parser.source.find(attachment.span()) {
        // Check if it is the 'main' base, and render it if true
        if child.parent_kind() != Some(SyntaxKind::MathAttach) {
            parser.state.is_base = true;
            parser.state.is_attachment = false;
            inner_ast_dfs(
                parser,
                attachment.base(),
                parser.uuid,
                parser.added_text_decoration,
                parser.offset,
            );
        } else {
            parser.state.is_base = false;
            parser.state.is_attachment = false;
            inner_ast_dfs(parser, attachment.base(), "", "", (0, 0));
        }
    }
    // Compute specific offset and style with rendering mode
    if parser.options.rendering_mode > 1 {
        parser.offset = (1, 0);
    }
    let top_decor = if parser.options.rendering_mode > 1 {
        "font-size: 0.8em; transform: translateY(-30%); display: inline-block;"
    } else {
        ""
    };
    let bottom_decor = if parser.options.rendering_mode > 1 {
        "font-size: 0.8em; transform: translateY(20%); display: inline-block;"
    } else {
        ""
    };
    // Set state for top and bottom attachment
    parser.state.is_base = false;
    parser.state.is_attachment = parser.options.rendering_mode > 1;
    if let Some(top) = attachment.top() {
        inner_ast_dfs(parser, top, "top-", top_decor, parser.offset)
    }
    if let Some(bottom) = attachment.bottom() {
        inner_ast_dfs(parser, bottom, "bottom-", bottom_decor, parser.offset)
    }
    // Restore the state
    parser.state.is_base = state.is_base;
    parser.state.is_attachment = state.is_attachment;
}

fn math_block(parser: &mut InnerParser) {
    let children: Vec<&SyntaxNode> = parser.expr.children().collect();
    // If we are in an attachment, check if the current math block is just paren around a symbol
    if children.len() == 3
        && children[0].kind() == SyntaxKind::LeftParen
        && children[1].kind() == SyntaxKind::Math
        && children[2].kind() == SyntaxKind::RightParen
    {
        // This serie of check aims to verify that the block inside paren is 'simple', wich means that we can propagate style (So top and bottom attachment)
        let mut propagate_style = false;
        let sub_children: Vec<&SyntaxNode> = children[1].children().collect();

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
                parser.options,
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
                parser.options,
            )
            .is_some()
            {
                propagate_style = true;
            }
        }

        // We can propagate, hide paren and then continue over children (With a for loop and a call to inner, to keep current style)
        if propagate_style {
            parser.insert_void(children[0].span(), (parser.offset.0, 0));
            parser.insert_void(children[2].span(), (0, parser.offset.1));
            for child in children[1].children() {
                if let Some(expr) = child.cast::<Expr>() {
                    inner_ast_dfs(
                        parser,
                        expr,
                        parser.uuid,
                        parser.added_text_decoration,
                        (0, 0),
                    );
                }
            }
            return;
        }
    }
    ast_dfs(parser, parser.expr, "", ""); // Propagate the function
}

fn shorthand_block(parser: &mut InnerParser) {
    let short = unchecked_cast_expr::<Shorthand>(parser.expr);
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
    parser.insert_result(
        short.span(),
        format!("{}-{}", parser.uuid, content.to_string()),
        content.to_string(),
        color,
        format!("{}{}", parser.added_text_decoration, decoration),
        parser.offset,
    );
}
fn text_block(parser: &mut InnerParser) {
    let text = unchecked_cast_expr::<Text>(parser.expr);
    if text.get().len() == 1 {
        if let Some((color, decoration)) = match text.get().as_str() {
            "+" => Some((Color::Operator, "")),
            "=" | "<" | ">" => Some((Color::Comparison, "")),
            "[" | "]" => Some((Color::Set, "")),
            _ => None,
        } {
            parser.insert_result(
                text.span(),
                format!("{}-{}", parser.uuid, text.get().to_string()),
                text.get().to_string(),
                color,
                format!("{}{}", parser.added_text_decoration, decoration),
                parser.offset,
            );
            return;
        }
    }
    if parser.state.is_attachment {
        parser.insert_result(
            text.span(),
            format!("{}-text-{}", parser.uuid, text.get().to_string()),
            text.get().to_string(),
            Color::Number,
            format!("{}", parser.added_text_decoration),
            parser.offset,
        );
    }
}
fn str_block(parser: &mut InnerParser) {
    let text = unchecked_cast_expr::<Str>(parser.expr);
    if parser.state.is_attachment {
        parser.insert_result(
            text.span(),
            format!("{}-text-{}", parser.uuid, text.get().to_string()),
            text.get().to_string(),
            Color::Number,
            format!("{}", parser.added_text_decoration),
            parser.offset,
        );
    }
}
fn func_call_block(parser: &mut InnerParser) {
    let func = unchecked_cast_expr::<FuncCall>(parser.expr);
    let args = func.args().to_untyped();
    let children: Vec<&SyntaxNode> = args.children().collect();
    let mut propagate_style = true;

    // If there is just a text, try to apply a text func like blackbold, caligraphy...
    if args.children().len() == 3
        && children[0].kind() == SyntaxKind::LeftParen
        && (children[1].kind() == SyntaxKind::Text || children[1].kind() == SyntaxKind::Str)
        && children[2].kind() == SyntaxKind::RightParen
        && parser.options.rendering_mode > 1
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
                    parser.insert_result(
                        text.span(),
                        format!("{}-{}", parser.uuid, symbol),
                        symbol,
                        Color::Number,
                        format!("{}{}", parser.added_text_decoration, decoration),
                        (
                            ident.as_str().len() + 1 + parser.offset.0,
                            1 + parser.offset.1,
                        ),
                    );
                    return;
                }
            }
            _ => {}
        }
    }
    if parser.options.rendering_mode > 2 {
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
                    && (children[1].kind() == SyntaxKind::MathIdent || children[1].kind() == SyntaxKind::Text || (children[1].kind() == SyntaxKind::MathAttach && children[1].children().len() == 3))
                    && children[2].kind() == SyntaxKind::RightParen
                {
                    parser.insert_result(span, format!("{}-func-{}", parser.uuid, symbol), symbol.to_string(), Color::Number, format!("{}", decoration), (0, 1));
                    parser.insert_void(children[2].span(), (0, 0));
                    propagate_style = false;
                }
            } else if let Some(symbol) = match content.as_str() {
                "abs" => Some('|'),
                "norm" => Some('‖'),
                _ => None,
            } {
                parser.insert_void(span, (parser.offset.0, 0));
                parser.insert_result(
                    children[0].span(),
                    format!("{}-func-{}", parser.uuid, symbol),
                    symbol.to_string(),
                    Color::Operator,
                    format!("{}", parser.added_text_decoration),
                    (0, 0),
                );
                parser.insert_result(
                    children.last().unwrap().span(),
                    format!("{}-func-{}", parser.uuid, symbol),
                    symbol.to_string(),
                    Color::Operator,
                    format!("{}", parser.added_text_decoration),
                    (0, parser.offset.1),
                );
            } else if content.as_str() == "sqrt" && args.children().len() == 3 && children[0].kind() == SyntaxKind::LeftParen && children[2].kind() == SyntaxKind::RightParen {
                let mut root_size = None;
                if children[1].kind() == SyntaxKind::MathIdent || children[1].kind() == SyntaxKind::Text {
                    root_size = Some(1.2);
                } else if children[1].kind() == SyntaxKind::MathAttach
                    && children[1].children().len() == 3
                    && (children[1].children().nth(2).unwrap().kind() == SyntaxKind::MathIdent || children[1].children().nth(2).unwrap().kind() == SyntaxKind::Text)
                {
                    root_size = Some(1.8);
                }
                if root_size.is_some() {
                    parser.insert_result(
                        children[0].span(),
                        format!("{}-func-{}-size-{}", parser.uuid, '\u{0305}', root_size.unwrap()),
                        '\u{0305}'.to_string(),
                        Color::Operator,
                        format!(
                            "font-family: JuliaMono; transform: scaleX({:.1}) translate(-0.01em, -0.25em); display: inline-block;",
                            root_size.unwrap()
                        ),
                        (0, 0),
                    );
                    parser.insert_result(
                        span,
                        format!("{}-func-{}", parser.uuid, '√'),
                        '√'.to_string(),
                        Color::Operator,
                        format!("font-family: JuliaMono; display: inline-block; transform: translate(0.1em, -0.1em);"),
                        (0, 0),
                    );
                    parser.insert_void(children[2].span(), (0, 0));
                    propagate_style = false;
                }
            } else {
                inner_ast_dfs(parser, func.callee(), parser.uuid, parser.added_text_decoration, parser.offset);
                propagate_style = false;
            }
        }
    } else {
        propagate_style = false;
    }
    ast_dfs(
        parser,
        func.args().to_untyped(),
        if propagate_style { parser.uuid } else { "" },
        if propagate_style {
            parser.added_text_decoration
        } else {
            ""
        },
    );
}

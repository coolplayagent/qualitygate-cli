use proc_macro2::{Span, TokenStream, TokenTree};
use std::collections::BTreeMap;
use syn::{
    Attribute, Item, Meta, Path, Token, UseTree,
    parse::Parser,
    punctuated::Punctuated,
    spanned::Spanned,
    visit::{self, Visit},
};

#[derive(Debug)]
pub(super) struct Reference {
    pub path: Vec<String>,
    pub line: usize,
}

#[derive(Default)]
pub(super) struct References {
    pub paths: Vec<Reference>,
    pub errors: Vec<String>,
    pub modules: Vec<(Vec<String>, syn::ItemMod)>,
    pub location: Vec<String>,
    aliases: Vec<BTreeMap<String, Vec<String>>>,
}

// Analyze every potentially active production configuration, including not(test).
// An unknown feature/target predicate is not evidence that an item is test-only.
fn production_predicate(meta: &Meta) -> Option<bool> {
    match meta {
        Meta::Path(path) if path.is_ident("test") => Some(false),
        Meta::List(list) => {
            let values = Punctuated::<Meta, Token![,]>::parse_terminated
                .parse2(list.tokens.clone())
                .ok()?
                .iter()
                .map(production_predicate)
                .collect::<Vec<_>>();
            if list.path.is_ident("not") && values.len() == 1 {
                values[0].map(|value| !value)
            } else if list.path.is_ident("all") {
                if values.contains(&Some(false)) {
                    Some(false)
                } else {
                    values
                        .iter()
                        .all(|value| *value == Some(true))
                        .then_some(true)
                }
            } else if list.path.is_ident("any") {
                if values.contains(&Some(true)) {
                    Some(true)
                } else {
                    values
                        .iter()
                        .all(|value| *value == Some(false))
                        .then_some(false)
                }
            } else {
                None
            }
        }
        _ => None,
    }
}

pub(super) fn test_only(attributes: &[Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        attribute.path().is_ident("cfg")
            && attribute
                .parse_args::<Meta>()
                .ok()
                .is_some_and(|meta| production_predicate(&meta) == Some(false))
    })
}

fn attributes(item: &Item) -> &[Attribute] {
    match item {
        Item::Const(value) => &value.attrs,
        Item::Enum(value) => &value.attrs,
        Item::ExternCrate(value) => &value.attrs,
        Item::Fn(value) => &value.attrs,
        Item::ForeignMod(value) => &value.attrs,
        Item::Impl(value) => &value.attrs,
        Item::Macro(value) => &value.attrs,
        Item::Mod(value) => &value.attrs,
        Item::Static(value) => &value.attrs,
        Item::Struct(value) => &value.attrs,
        Item::Trait(value) => &value.attrs,
        Item::TraitAlias(value) => &value.attrs,
        Item::Type(value) => &value.attrs,
        Item::Union(value) => &value.attrs,
        Item::Use(value) => &value.attrs,
        _ => &[],
    }
}

fn imports(tree: &UseTree, prefix: Vec<String>, output: &mut Vec<(Vec<String>, String, Span)>) {
    match tree {
        UseTree::Path(value) => {
            let mut prefix = prefix;
            prefix.push(value.ident.to_string());
            imports(&value.tree, prefix, output);
        }
        UseTree::Group(value) => {
            for item in &value.items {
                imports(item, prefix.clone(), output);
            }
        }
        UseTree::Name(value) => {
            let mut path = prefix;
            let alias = if value.ident == "self" {
                path.last().cloned().unwrap_or_default()
            } else {
                path.push(value.ident.to_string());
                value.ident.to_string()
            };
            output.push((path, alias, value.span()));
        }
        UseTree::Rename(value) => {
            let mut path = prefix;
            if value.ident != "self" {
                path.push(value.ident.to_string());
            }
            output.push((path, value.rename.to_string(), value.span()));
        }
        UseTree::Glob(value) => {
            let mut path = prefix;
            path.push("*".into());
            output.push((path, String::new(), value.span()));
        }
    }
}

impl References {
    pub fn analyze(items: &[Item], location: Vec<String>) -> Self {
        let mut result = Self {
            location,
            ..Self::default()
        };
        result.items(items);
        result
    }

    fn items(&mut self, items: &[Item]) {
        let mut aliases = BTreeMap::new();
        for item in items.iter().filter(|item| !test_only(attributes(item))) {
            if let Item::Use(value) = item {
                let mut values = Vec::new();
                imports(&value.tree, vec![], &mut values);
                for (path, alias, _) in values {
                    if !alias.is_empty() {
                        aliases.insert(alias, path);
                    }
                }
            }
        }
        self.aliases.push(aliases);
        for item in items {
            self.visit_item(item);
        }
        self.aliases.pop();
    }

    fn record(&mut self, mut path: Vec<String>, span: Span) {
        // Imports already establish module edges; resolving aliases also exposes
        // grouped/renamed standard-library capabilities such as sys::env.
        let mut expanded = std::collections::BTreeSet::new();
        while let Some(first) = path.first() {
            if matches!(first.as_str(), "crate" | "self" | "super" | "qualitygate") {
                break;
            }
            if !expanded.insert(first.clone()) {
                break;
            }
            let Some(alias) = self.aliases.iter().rev().find_map(|scope| scope.get(first)) else {
                break;
            };
            if alias.first() == Some(first) {
                break;
            }
            path = alias.iter().chain(path.iter().skip(1)).cloned().collect();
        }
        match path.first().map(String::as_str) {
            Some("crate" | "qualitygate") => {
                path[0] = "crate".into();
            }
            Some("self" | "super") => {
                let mut module = self.location.clone();
                let mut index = 0;
                while let Some(part) = path.get(index) {
                    match part.as_str() {
                        "self" if index == 0 => {}
                        "super" => {
                            if module.pop().is_none() {
                                self.errors.push(format!(
                                    "line {}: relative path escapes crate",
                                    span.start().line
                                ));
                                return;
                            }
                        }
                        _ => break,
                    }
                    index += 1;
                }
                path = std::iter::once("crate".into())
                    .chain(module)
                    .chain(path.into_iter().skip(index))
                    .collect();
            }
            _ => {}
        }
        self.paths.push(Reference {
            path,
            line: span.start().line,
        });
    }

    fn tokens(&mut self, stream: TokenStream) {
        let tokens: Vec<_> = stream.into_iter().collect();
        for (index, token) in tokens.iter().enumerate() {
            if let TokenTree::Group(group) = token {
                self.tokens(group.stream());
            }
            let TokenTree::Ident(first) = token else {
                continue;
            };
            if first == "use"
                && let Some(end) = tokens[index..].iter().position(
                    |token| matches!(token, TokenTree::Punct(value) if value.as_char() == ';'),
                )
                && let Ok(item) = syn::parse2::<syn::ItemUse>(
                    tokens[index..=index + end].iter().cloned().collect(),
                )
            {
                self.visit_item_use(&item);
            }
            if (first == "include" || first == "macro_rules")
                && matches!(tokens.get(index + 1), Some(TokenTree::Punct(value)) if value.as_char() == '!')
            {
                self.errors.push(format!("line {}: generated production source inside macro needs explicit graph support", first.span().start().line));
            }
            let mut path = vec![first.to_string()];
            let mut next = index + 1;
            while let [
                TokenTree::Punct(a),
                TokenTree::Punct(b),
                TokenTree::Ident(part),
                ..,
            ] = &tokens[next..]
            {
                if a.as_char() != ':' || b.as_char() != ':' {
                    break;
                }
                path.push(part.to_string());
                next += 3;
            }
            if path.len() > 1 {
                self.record(path, first.span());
            }
        }
    }
}

impl<'ast> Visit<'ast> for References {
    fn visit_item(&mut self, item: &'ast Item) {
        if test_only(attributes(item)) {
            return;
        }
        if let Item::Verbatim(_) = item {
            self.errors.push("unparsed production item".into());
        }
        visit::visit_item(self, item);
    }

    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        if item.attrs.iter().any(|attribute| {
            attribute.path().is_ident("path")
                || (attribute.path().is_ident("cfg_attr")
                    && matches!(&attribute.meta, Meta::List(list) if list.tokens.to_string().split(|c: char| !c.is_alphanumeric() && c != '_').any(|word| word == "path")))
        }) {
            self.errors.push(format!("line {}: production module path redirects need explicit graph support", item.span().start().line));
            return;
        }
        let mut location = self.location.clone();
        location.push(item.ident.to_string());
        if let Some((_, items)) = &item.content {
            let previous = std::mem::replace(&mut self.location, location);
            self.items(items);
            self.location = previous;
        } else {
            self.modules.push((location, item.clone()));
        }
    }

    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        let mut paths = Vec::new();
        imports(&item.tree, vec![], &mut paths);
        for (path, _, span) in paths {
            self.record(path, span);
        }
    }

    fn visit_block(&mut self, block: &'ast syn::Block) {
        let items = block
            .stmts
            .iter()
            .filter_map(|statement| {
                if let syn::Stmt::Item(item) = statement {
                    Some(item.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        // Block imports have their own lexical scope and can precede their use.
        let mut aliases = BTreeMap::new();
        for item in &items {
            if let Item::Use(item) = item
                && !test_only(&item.attrs)
            {
                let mut values = Vec::new();
                imports(&item.tree, vec![], &mut values);
                for (path, alias, _) in values {
                    if !alias.is_empty() {
                        aliases.insert(alias, path);
                    }
                }
            }
        }
        self.aliases.push(aliases);
        visit::visit_block(self, block);
        self.aliases.pop();
    }

    fn visit_path(&mut self, path: &'ast Path) {
        self.record(
            path.segments
                .iter()
                .map(|part| part.ident.to_string())
                .collect(),
            path.span(),
        );
        visit::visit_path(self, path);
    }

    fn visit_macro(&mut self, value: &'ast syn::Macro) {
        if value.path.is_ident("include") || value.path.is_ident("macro_rules") {
            self.errors.push(format!(
                "line {}: generated production source needs explicit graph support",
                value.span().start().line
            ));
        }
        self.visit_path(&value.path);
        self.tokens(value.tokens.clone());
    }

    fn visit_item_extern_crate(&mut self, item: &'ast syn::ItemExternCrate) {
        self.errors.push(format!(
            "line {}: extern crate aliases need explicit graph support",
            item.span().start().line
        ));
    }
}

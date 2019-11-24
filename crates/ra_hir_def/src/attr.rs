//! A higher level attributes based on TokenTree, with also some shortcuts.

use std::{ops, sync::Arc};

use hir_expand::{either::Either, hygiene::Hygiene, AstId, Source};
use mbe::ast_to_token_tree;
use ra_cfg::CfgOptions;
use ra_syntax::{
    ast::{self, AstNode, AttrsOwner},
    SmolStr,
};
use tt::Subtree;

use crate::{
    db::DefDatabase, path::Path, AdtId, AstItemDef, AttrDefId, HasChildSource, HasSource, Lookup,
};

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct Attrs {
    entries: Option<Arc<[Attr]>>,
}

impl ops::Deref for Attrs {
    type Target = [Attr];

    fn deref(&self) -> &[Attr] {
        match &self.entries {
            Some(it) => &*it,
            None => &[],
        }
    }
}

impl Attrs {
    pub(crate) fn attrs_query(db: &impl DefDatabase, def: AttrDefId) -> Attrs {
        match def {
            AttrDefId::ModuleId(module) => {
                let def_map = db.crate_def_map(module.krate);
                let src = match def_map[module.module_id].declaration_source(db) {
                    Some(it) => it,
                    None => return Attrs::default(),
                };
                Attrs::from_attrs_owner(db, src.as_ref().map(|it| it as &dyn AttrsOwner))
            }
            AttrDefId::StructFieldId(it) => {
                let src = it.parent.child_source(db);
                match &src.value[it.local_id] {
                    Either::A(_tuple) => Attrs::default(),
                    Either::B(record) => Attrs::from_attrs_owner(db, src.with_value(record)),
                }
            }
            AttrDefId::EnumVariantId(var_id) => {
                let src = var_id.parent.child_source(db);
                let src = src.as_ref().map(|it| &it[var_id.local_id]);
                Attrs::from_attrs_owner(db, src.map(|it| it as &dyn AttrsOwner))
            }
            AttrDefId::AdtId(it) => match it {
                AdtId::StructId(it) => attrs_from_ast(it.0.lookup_intern(db).ast_id, db),
                AdtId::EnumId(it) => attrs_from_ast(it.lookup_intern(db).ast_id, db),
                AdtId::UnionId(it) => attrs_from_ast(it.0.lookup_intern(db).ast_id, db),
            },
            AttrDefId::TraitId(it) => attrs_from_ast(it.lookup_intern(db).ast_id, db),
            AttrDefId::MacroDefId(it) => attrs_from_ast(it.ast_id, db),
            AttrDefId::ImplId(it) => attrs_from_ast(it.lookup_intern(db).ast_id, db),
            AttrDefId::ConstId(it) => attrs_from_loc(it.lookup(db), db),
            AttrDefId::StaticId(it) => attrs_from_loc(it.lookup(db), db),
            AttrDefId::FunctionId(it) => attrs_from_loc(it.lookup(db), db),
            AttrDefId::TypeAliasId(it) => attrs_from_loc(it.lookup(db), db),
        }
    }

    fn from_attrs_owner(db: &impl DefDatabase, owner: Source<&dyn AttrsOwner>) -> Attrs {
        let hygiene = Hygiene::new(db, owner.file_id);
        Attrs::new(owner.value, &hygiene)
    }

    pub(crate) fn new(owner: &dyn AttrsOwner, hygiene: &Hygiene) -> Attrs {
        let mut attrs = owner.attrs().peekable();
        let entries = if attrs.peek().is_none() {
            // Avoid heap allocation
            None
        } else {
            Some(attrs.flat_map(|ast| Attr::from_src(ast, hygiene)).collect())
        };
        Attrs { entries }
    }

    pub fn has_atom(&self, atom: &str) -> bool {
        self.iter().any(|it| it.is_simple_atom(atom))
    }

    pub fn find_string_value(&self, key: &str) -> Option<SmolStr> {
        self.iter().filter(|attr| attr.is_simple_atom(key)).find_map(|attr| {
            match attr.input.as_ref()? {
                AttrInput::Literal(it) => Some(it.clone()),
                _ => None,
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attr {
    pub(crate) path: Path,
    pub(crate) input: Option<AttrInput>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttrInput {
    Literal(SmolStr),
    TokenTree(Subtree),
}

impl Attr {
    fn from_src(ast: ast::Attr, hygiene: &Hygiene) -> Option<Attr> {
        let path = Path::from_src(ast.path()?, hygiene)?;
        let input = match ast.input() {
            None => None,
            Some(ast::AttrInput::Literal(lit)) => {
                // FIXME: escape? raw string?
                let value = lit.syntax().first_token()?.text().trim_matches('"').into();
                Some(AttrInput::Literal(value))
            }
            Some(ast::AttrInput::TokenTree(tt)) => {
                Some(AttrInput::TokenTree(ast_to_token_tree(&tt)?.0))
            }
        };

        Some(Attr { path, input })
    }

    pub fn is_simple_atom(&self, name: &str) -> bool {
        // FIXME: Avoid cloning
        self.path.as_ident().map_or(false, |s| s.to_string() == name)
    }

    // FIXME: handle cfg_attr :-)
    pub fn as_cfg(&self) -> Option<&Subtree> {
        if !self.is_simple_atom("cfg") {
            return None;
        }
        match &self.input {
            Some(AttrInput::TokenTree(subtree)) => Some(subtree),
            _ => None,
        }
    }

    pub(crate) fn is_cfg_enabled(&self, cfg_options: &CfgOptions) -> Option<bool> {
        cfg_options.is_cfg_enabled(self.as_cfg()?)
    }
}

fn attrs_from_ast<D, N>(src: AstId<N>, db: &D) -> Attrs
where
    N: ast::AttrsOwner,
    D: DefDatabase,
{
    let src = Source::new(src.file_id(), src.to_node(db));
    Attrs::from_attrs_owner(db, src.as_ref().map(|it| it as &dyn AttrsOwner))
}

fn attrs_from_loc<T, D>(node: T, db: &D) -> Attrs
where
    T: HasSource,
    T::Value: ast::AttrsOwner,
    D: DefDatabase,
{
    let src = node.source(db);
    Attrs::from_attrs_owner(db, src.as_ref().map(|it| it as &dyn AttrsOwner))
}

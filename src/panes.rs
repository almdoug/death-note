//! Tela dividida dentro de uma única aba.
//!
//! A barra de abas tem uma faixa só. Uma aba é uma nota ou um *grupo*: várias
//! notas lado a lado, organizadas numa árvore de painéis (`Pane`). Quando o
//! usuário arrasta uma aba para a lateral, o egui_dock divide a área; logo em
//! seguida essa divisão é dobrada num grupo (ver `fold`). Desfazer a divisão
//! devolve cada nota para a sua própria aba.

use crate::storage::NoteId;
use egui_dock::{Node, NodeIndex, Tree};
use serde::{Deserialize, Serialize};

pub type GroupId = u64;

/// Uma aba da barra: uma nota ou um grupo de notas em tela dividida.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tab {
    Note(NoteId),
    Group(GroupId),
}

/// Árvore de painéis de um grupo.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum Pane {
    Note(NoteId),
    Split {
        /// `true`: lado a lado (esquerda | direita); `false`: em cima / embaixo.
        side_by_side: bool,
        /// Fração ocupada pelo primeiro painel.
        fraction: f32,
        first: Box<Pane>,
        second: Box<Pane>,
    },
}

impl Pane {
    pub fn split(side_by_side: bool, first: Pane, second: Pane) -> Pane {
        Pane::Split {
            side_by_side,
            fraction: 0.5,
            first: Box::new(first),
            second: Box::new(second),
        }
    }

    /// Notas na ordem de leitura (esquerda → direita, cima → baixo).
    pub fn notes(&self) -> Vec<NoteId> {
        let mut out = Vec::new();
        self.collect(&mut out);
        out
    }

    fn collect(&self, out: &mut Vec<NoteId>) {
        match self {
            Pane::Note(id) => out.push(*id),
            Pane::Split { first, second, .. } => {
                first.collect(out);
                second.collect(out);
            }
        }
    }

    pub fn contains(&self, id: NoteId) -> bool {
        match self {
            Pane::Note(n) => *n == id,
            Pane::Split { first, second, .. } => first.contains(id) || second.contains(id),
        }
    }

    pub fn first_note(&self) -> NoteId {
        match self {
            Pane::Note(n) => *n,
            Pane::Split { first, .. } => first.first_note(),
        }
    }

    /// Mantém só as notas aceitas por `keep`; `None` se não sobrar nenhuma.
    /// Um divisor que perde um dos lados vira o lado que sobrou.
    pub fn retain(self, keep: &impl Fn(NoteId) -> bool) -> Option<Pane> {
        match self {
            Pane::Note(n) => keep(n).then_some(Pane::Note(n)),
            Pane::Split {
                side_by_side,
                fraction,
                first,
                second,
            } => match (first.retain(keep), second.retain(keep)) {
                (Some(a), Some(b)) => Some(Pane::Split {
                    side_by_side,
                    fraction,
                    first: Box::new(a),
                    second: Box::new(b),
                }),
                (Some(x), None) | (None, Some(x)) => Some(x),
                (None, None) => None,
            },
        }
    }

    /// Tira uma nota da árvore.
    pub fn remove(self, id: NoteId) -> Option<Pane> {
        self.retain(&|n| n != id)
    }

    /// Troca a nota `old` por `new` no mesmo painel.
    pub fn replace(&mut self, old: NoteId, new: NoteId) -> bool {
        match self {
            Pane::Note(n) if *n == old => {
                *n = new;
                true
            }
            Pane::Note(_) => false,
            Pane::Split { first, second, .. } => first.replace(old, new) || second.replace(old, new),
        }
    }

    /// Divide o painel da nota `id`, pondo `new` depois (à direita ou abaixo).
    pub fn split_at(&mut self, id: NoteId, new: NoteId, side_by_side: bool) -> bool {
        match self {
            Pane::Note(n) if *n == id => {
                *self = Pane::split(side_by_side, Pane::Note(id), Pane::Note(new));
                true
            }
            Pane::Note(_) => false,
            Pane::Split { first, second, .. } => {
                first.split_at(id, new, side_by_side) || second.split_at(id, new, side_by_side)
            }
        }
    }
}

/// Grupo de notas em tela dividida, mostrado como uma aba só.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Group {
    pub root: Pane,
    /// Nota em foco dentro do grupo (volta a ela ao reabrir a aba).
    pub focus: NoteId,
}

impl Group {
    pub fn new(root: Pane, focus: NoteId) -> Self {
        let focus = if root.contains(focus) { focus } else { root.first_note() };
        Self { root, focus }
    }
}

/// Aba como fica salva na sessão.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum SavedTab {
    Note(NoteId),
    Group(Group),
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct SavedTabs {
    pub tabs: Vec<SavedTab>,
    #[serde(default)]
    pub active: usize,
}

/// Posição de cada aba depois de dobrar uma divisão do egui_dock.
#[derive(Debug, PartialEq)]
pub enum Slot<T> {
    Tab(T),
    /// Lugar onde fica a aba do grupo novo.
    Group,
}

/// Dobra uma árvore dividida do egui_dock num único grupo.
///
/// A aba ativa de cada painel entra no grupo (`to_pane` converte; um grupo
/// existente é incorporado inteiro). As demais abas continuam na barra, na
/// ordem em que estavam; o grupo fica no lugar da aba ativa do primeiro painel.
pub fn fold<T: Copy>(tree: &Tree<T>, to_pane: &mut impl FnMut(T) -> Option<Pane>) -> (Option<Pane>, Vec<Slot<T>>) {
    let mut order = Vec::new();
    let root = walk(tree, NodeIndex::root(), to_pane, &mut order);
    (root, order)
}

fn walk<T: Copy>(
    tree: &Tree<T>,
    idx: NodeIndex,
    to_pane: &mut impl FnMut(T) -> Option<Pane>,
    order: &mut Vec<Slot<T>>,
) -> Option<Pane> {
    if idx.0 >= tree.len() {
        return None;
    }
    match &tree[idx] {
        Node::Empty => None,
        Node::Leaf(leaf) => {
            if leaf.tabs.is_empty() {
                return None;
            }
            let active = leaf.active.0.min(leaf.tabs.len() - 1);
            let mut pane = None;
            for (i, t) in leaf.tabs.iter().enumerate() {
                if i == active {
                    pane = to_pane(*t);
                    if pane.is_none() {
                        order.push(Slot::Tab(*t));
                    } else if !order.iter().any(|s| matches!(s, Slot::Group)) {
                        order.push(Slot::Group);
                    }
                } else {
                    order.push(Slot::Tab(*t));
                }
            }
            pane
        }
        Node::Horizontal(s) | Node::Vertical(s) => {
            let side_by_side = matches!(tree[idx], Node::Horizontal(_));
            let fraction = s.fraction.clamp(0.05, 0.95);
            let a = walk(tree, idx.left(), to_pane, order);
            let b = walk(tree, idx.right(), to_pane, order);
            match (a, b) {
                (Some(a), Some(b)) => Some(Pane::Split {
                    side_by_side,
                    fraction,
                    first: Box::new(a),
                    second: Box::new(b),
                }),
                (Some(x), None) | (None, Some(x)) => Some(x),
                (None, None) => None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_dock::{DockState, Split};

    fn n(id: NoteId) -> Pane {
        Pane::Note(id)
    }

    #[test]
    fn remove_collapses_split() {
        let p = Pane::split(true, n(1), Pane::split(false, n(2), n(3)));
        let p = p.remove(2).unwrap();
        assert_eq!(p, Pane::split(true, n(1), n(3)));
        let p = p.remove(1).unwrap();
        assert_eq!(p, n(3));
        assert_eq!(p.remove(3), None);
    }

    #[test]
    fn split_and_replace() {
        let mut p = n(1);
        assert!(p.split_at(1, 2, true));
        assert!(p.split_at(2, 3, false));
        assert_eq!(p.notes(), vec![1, 2, 3]);
        assert!(p.replace(3, 9));
        assert!(p.contains(9) && !p.contains(3));
        assert!(!p.split_at(42, 5, true));
    }

    #[test]
    fn fold_dock_split_into_group() {
        // [1, 2*, 3] | [4*]  →  [1, grupo(2|4), 3]
        let mut d = DockState::new(vec![1u64, 2, 3]);
        let tree = d.main_surface_mut();
        let _ = tree.set_active_tab(NodeIndex::root(), egui_dock::TabIndex(1));
        tree.split(NodeIndex::root(), Split::Right, 0.5, Node::leaf(4u64));
        let (root, order) = fold(d.main_surface(), &mut |t| Some(n(t)));
        assert_eq!(root, Some(Pane::split(true, n(2), n(4))));
        assert_eq!(order, vec![Slot::Tab(1), Slot::Group, Slot::Tab(3)]);
    }

    #[test]
    fn fold_merges_existing_group() {
        // Uma aba arrastada para baixo de um grupo entra nele.
        let mut d = DockState::new(vec![10u64]);
        d.main_surface_mut()
            .split(NodeIndex::root(), Split::Below, 0.5, Node::leaf(5u64));
        let group = Pane::split(true, n(1), n(2));
        let (root, order) = fold(d.main_surface(), &mut |t| {
            Some(if t == 10 { group.clone() } else { n(t) })
        });
        assert_eq!(root, Some(Pane::split(false, group, n(5))));
        assert_eq!(order, vec![Slot::Group]);
    }

    #[test]
    fn saved_tabs_roundtrip() {
        let s = SavedTabs {
            tabs: vec![
                SavedTab::Note(1),
                SavedTab::Group(Group::new(Pane::split(true, n(2), n(3)), 3)),
            ],
            active: 1,
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: SavedTabs = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }
}

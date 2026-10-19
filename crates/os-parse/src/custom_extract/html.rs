//! Retain source attribute order without changing the shared parser's selector
//! lookup invariants (scraper stores attributes sorted by name).
use ego_tree::{iter::Edge, NodeId};
use html5ever::{
    driver::{parse_document, ParseOpts},
    serialize::{Serialize, SerializeOpts, Serializer, TraversalScope},
    tendril::{StrTendril, TendrilSink},
    tree_builder::{ElementFlags, NodeOrText, QuirksMode, TreeBuilderOpts, TreeSink},
    Attribute, QualName,
};
use scraper::{ElementRef, Html, HtmlTreeSink, Node};
use std::{
    borrow::Cow,
    cell::{Ref, RefCell},
    collections::HashMap,
    io,
};

type Orders = HashMap<NodeId, Vec<QualName>>;
pub(super) struct OrderedHtml {
    pub html: Html,
    orders: Orders,
}

impl OrderedHtml {
    pub fn parse(source: &str) -> Self {
        parse_document(
            Sink {
                inner: HtmlTreeSink::new(Html::new_document()),
                orders: RefCell::new(HashMap::new()),
            },
            ParseOpts {
                tree_builder: TreeBuilderOpts {
                    scripting_enabled: false,
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .one(source)
    }
    pub fn serialize(&self, element: ElementRef<'_>, inner: bool) -> String {
        let mut bytes = Vec::new();
        let scope = if inner {
            TraversalScope::ChildrenOnly(Some(element.value().name.clone()))
        } else {
            TraversalScope::IncludeNode
        };
        html5ever::serialize(
            &mut bytes,
            &OrderedElement {
                element,
                orders: &self.orders,
            },
            SerializeOpts {
                traversal_scope: scope,
                ..Default::default()
            },
        )
        .expect("serialize to memory");
        String::from_utf8(bytes).expect("HTML serializer emits UTF-8")
    }
}
struct OrderedElement<'a> {
    element: ElementRef<'a>,
    orders: &'a Orders,
}
impl Serialize for OrderedElement<'_> {
    fn serialize<S: Serializer>(
        &self,
        serializer: &mut S,
        scope: TraversalScope,
    ) -> io::Result<()> {
        for edge in self.element.traverse() {
            let node = match edge {
                Edge::Open(n) | Edge::Close(n) => n,
            };
            if matches!(scope, TraversalScope::ChildrenOnly(_)) && node.id() == self.element.id() {
                continue;
            }
            match edge {
                Edge::Open(node) => match node.value() {
                    Node::Element(e) => {
                        let mut attrs: Vec<_> = e.attrs.iter().collect();
                        if let Some(order) = self.orders.get(&node.id()) {
                            attrs.sort_by_key(|(name, _)| {
                                order.iter().position(|n| n == name).unwrap_or(usize::MAX)
                            });
                        }
                        serializer.start_elem(
                            e.name.clone(),
                            attrs
                                .into_iter()
                                .map(|(name, value)| (name, value.as_ref())),
                        )?;
                    }
                    Node::Text(t) => serializer.write_text(t)?,
                    Node::Comment(c) => serializer.write_comment(c)?,
                    _ => {}
                },
                Edge::Close(node) => {
                    if let Node::Element(e) = node.value() {
                        serializer.end_elem(e.name.clone())?;
                    }
                }
            }
        }
        Ok(())
    }
}
struct Sink {
    inner: HtmlTreeSink,
    orders: RefCell<Orders>,
}
impl TreeSink for Sink {
    type Output = OrderedHtml;
    type Handle = NodeId;
    type ElemName<'a> = Ref<'a, QualName>;
    fn finish(self) -> OrderedHtml {
        OrderedHtml {
            html: self.inner.finish(),
            orders: self.orders.into_inner(),
        }
    }
    fn create_element(&self, name: QualName, attrs: Vec<Attribute>, flags: ElementFlags) -> NodeId {
        let order = attrs.iter().map(|a| a.name.clone()).collect();
        let node = self.inner.create_element(name, attrs, flags);
        self.orders.borrow_mut().insert(node, order);
        node
    }
    fn add_attrs_if_missing(&self, target: &NodeId, attrs: Vec<Attribute>) {
        let mut orders = self.orders.borrow_mut();
        let order = orders.entry(*target).or_default();
        for attr in &attrs {
            if !order.contains(&attr.name) {
                order.push(attr.name.clone());
            }
        }
        self.inner.add_attrs_if_missing(target, attrs);
    }
    fn parse_error(&self, msg: Cow<'static, str>) {
        self.inner.parse_error(msg);
    }
    fn set_quirks_mode(&self, mode: QuirksMode) {
        self.inner.set_quirks_mode(mode);
    }
    fn get_document(&self) -> NodeId {
        self.inner.get_document()
    }
    fn same_node(&self, x: &NodeId, y: &NodeId) -> bool {
        self.inner.same_node(x, y)
    }
    fn elem_name<'a>(&'a self, target: &'a NodeId) -> Ref<'a, QualName> {
        self.inner.elem_name(target)
    }
    fn create_comment(&self, text: StrTendril) -> NodeId {
        self.inner.create_comment(text)
    }
    fn append_doctype_to_document(
        &self,
        name: StrTendril,
        public_id: StrTendril,
        system_id: StrTendril,
    ) {
        self.inner
            .append_doctype_to_document(name, public_id, system_id);
    }
    fn append(&self, parent: &NodeId, child: NodeOrText<NodeId>) {
        self.inner.append(parent, child);
    }
    fn append_before_sibling(&self, sibling: &NodeId, child: NodeOrText<NodeId>) {
        self.inner.append_before_sibling(sibling, child);
    }
    fn remove_from_parent(&self, target: &NodeId) {
        self.inner.remove_from_parent(target);
    }
    fn reparent_children(&self, node: &NodeId, new_parent: &NodeId) {
        self.inner.reparent_children(node, new_parent);
    }
    fn get_template_contents(&self, target: &NodeId) -> NodeId {
        self.inner.get_template_contents(target)
    }
    fn create_pi(&self, target: StrTendril, data: StrTendril) -> NodeId {
        self.inner.create_pi(target, data)
    }
    fn append_based_on_parent_node(
        &self,
        element: &NodeId,
        prev: &NodeId,
        child: NodeOrText<NodeId>,
    ) {
        self.inner.append_based_on_parent_node(element, prev, child);
    }
}

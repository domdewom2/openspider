//! A page-local XDM mirror of the HTML5 tree. HTML elements are unnamespaced for
//! conventional HTML XPath; foreign content retains its namespace. No URI loader.
use os_model::{Extractor, ExtractorOutput};
use scraper::{ElementRef, Html, Node};
use xee_xpath::{Documents, Item, Queries, Query};

use super::{element_value, html::OrderedHtml};

pub(super) struct Document<'a> {
    documents: Documents,
    root: Item,
    elements: Vec<(Item, ElementRef<'a>)>,
}

impl<'a> Document<'a> {
    pub fn new(html: &'a Html) -> Self {
        let mut documents = Documents::new();
        let xot = documents.xot_mut();
        let root = xot.new_document();
        let mut stack = vec![root];
        let mut elements = Vec::new();
        for edge in html.tree.root().traverse() {
            match edge {
                ego_tree::iter::Edge::Open(node) => {
                    let parent = *stack.last().unwrap();
                    let child = match node.value() {
                        Node::Element(e) => {
                            let name = if e.name.ns.as_ref() == crate::dom::XHTML_NS {
                                xot.add_name(e.name())
                            } else {
                                let ns = xot.add_namespace(&e.name.ns);
                                xot.add_name_ns(e.name(), ns)
                            };
                            let child = xot.new_element(name);
                            for (name, value) in &e.attrs {
                                let name = if name.ns.is_empty() {
                                    xot.add_name(&name.local)
                                } else {
                                    let ns = xot.add_namespace(&name.ns);
                                    xot.add_name_ns(&name.local, ns)
                                };
                                xot.set_attribute(child, name, value.to_string());
                            }
                            elements.push((Item::Node(child), ElementRef::wrap(node).unwrap()));
                            child
                        }
                        Node::Text(t) => xot.new_text(t),
                        Node::Comment(c) => xot.new_comment(c),
                        _ => continue,
                    };
                    xot.append(parent, child).expect("HTML tree parent");
                    stack.push(child);
                }
                ego_tree::iter::Edge::Close(node) => {
                    if matches!(
                        node.value(),
                        Node::Element(_) | Node::Text(_) | Node::Comment(_)
                    ) {
                        stack.pop();
                    }
                }
            }
        }
        Self {
            documents,
            root: Item::Node(root),
            elements,
        }
    }

    pub fn evaluate(
        &mut self,
        extractor: &Extractor,
        limit: usize,
        ordered: Option<&OrderedHtml>,
    ) -> Result<Vec<String>, String> {
        let query = Queries::default()
            .sequence(&extractor.expression)
            .map_err(|e| format!("XPath: {e}"))?;
        let sequence = query
            .execute(&mut self.documents, &self.root)
            .map_err(|e| format!("XPath: {e}"))?;
        let limit = if extractor.output == ExtractorOutput::FunctionValue {
            1
        } else {
            limit
        };
        sequence
            .iter()
            .take(limit)
            .map(|item| {
                if extractor.output != ExtractorOutput::FunctionValue {
                    if let Ok(node) = item.to_node() {
                        if let Some((_, element)) = self
                            .elements
                            .iter()
                            .find(|(item, _)| item.to_node().ok() == Some(node))
                        {
                            return Ok(element_value(*element, extractor.output, ordered));
                        }
                    }
                }
                item.string_value(self.documents.xot())
                    .map_err(|e| format!("XPath: {e}"))
            })
            .collect()
    }
}

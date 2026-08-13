// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What the server says it can be filtered and sorted by.

use afisharr_sources::outbound::Method;
use serde::Deserialize;

use crate::{
    libraries::{ItemKind, ItemQuery, SectionKey, Window},
    server::{PlexServerClient, ServerError},
};

/// One filter the server offers on a library type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredFilter {
    /// The field the filter is on, e.g. `genre`.
    pub filter: String,
    /// The declared type of the value, e.g. `string`, `integer`, `boolean`.
    pub filter_type: String,
    /// The label the server shows for it.
    pub title: Option<String>,
    /// The endpoint that lists this filter's enumerated choices, when it has
    /// them. `None` means the filter takes a free value rather than a choice.
    pub key: Option<String>,
}

/// One sort the server offers on a library type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredSort {
    /// The sort key, e.g. `titleSort`.
    pub key: String,
    /// The label the server shows for it.
    pub title: Option<String>,
    /// Which direction the server sorts by default, when it says.
    pub default_direction: Option<String>,
}

/// One field the server declares, with the type that decides its operators.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredField {
    /// The field key, e.g. `audioLanguage`.
    pub key: String,
    /// The declared type, which indexes into the operator table.
    pub field_type: String,
    /// The subtype, e.g. `rating`, `decade`, when the server declares one.
    pub sub_type: Option<String>,
    /// The label the server shows for it.
    pub title: Option<String>,
}

/// One operator the server declares legal for a field type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldOperator {
    /// The operator key, exactly as the server spells it — `=`, `!=`, `>>=`.
    pub key: String,
    /// The label the server shows for it.
    pub title: Option<String>,
}

/// The operators legal for one declared field type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldType {
    /// The type these operators apply to.
    pub field_type: String,
    /// The operators, exactly as the server declares them.
    pub operators: Vec<FieldOperator>,
}

/// Everything the server declares for one library type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilteringType {
    /// The library type this describes, when this build models it.
    pub libtype: Option<ItemKind>,
    /// The raw type as the server spells it, kept whether or not it is modelled.
    pub raw_type: String,
    /// The filters offered.
    pub filters: Vec<DiscoveredFilter>,
    /// The sorts offered.
    pub sorts: Vec<DiscoveredSort>,
    /// The fields declared.
    pub fields: Vec<DiscoveredField>,
}

/// One library's declared vocabulary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vocabulary {
    /// One entry per library type the server described.
    pub types: Vec<FilteringType>,
    /// The operator table, indexed by field type.
    pub field_types: Vec<FieldType>,
}

impl Vocabulary {
    /// The operators the server declares legal for `field_type`.
    ///
    /// `None` is "the server did not describe this type", which is not "no
    /// operator is legal": a predicate against an undescribed type falls back
    /// to local evaluation, and one against a type declared with an empty
    /// operator list is genuinely unfilterable (P1).
    #[must_use]
    pub fn operators_for(&self, field_type: &str) -> Option<&[FieldOperator]> {
        self.field_types
            .iter()
            .find(|entry| entry.field_type == field_type)
            .map(|entry| entry.operators.as_slice())
    }
}

/// The `Meta` block Plex answers when asked to describe itself.
#[derive(Debug, Deserialize)]
pub(crate) struct MetaBody {
    #[serde(default, rename = "Meta")]
    pub(crate) meta: Option<MetaInner>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct MetaInner {
    #[serde(default, rename = "Type")]
    types: Vec<TypeBody>,
    #[serde(default, rename = "FieldType")]
    field_types: Vec<FieldTypeBody>,
}

#[derive(Debug, Deserialize)]
struct TypeBody {
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default, rename = "Filter")]
    filter: Vec<FilterBody>,
    #[serde(default, rename = "Sort")]
    sort: Vec<SortBody>,
    #[serde(default, rename = "Field")]
    field: Vec<FieldBody>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FilterBody {
    #[serde(default)]
    filter: Option<String>,
    #[serde(default)]
    filter_type: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    key: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SortBody {
    #[serde(default)]
    key: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    default_direction: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FieldBody {
    #[serde(default)]
    key: Option<String>,
    #[serde(default, rename = "type")]
    field_type: Option<String>,
    #[serde(default)]
    sub_type: Option<String>,
    #[serde(default)]
    title: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FieldTypeBody {
    #[serde(default, rename = "type")]
    field_type: Option<String>,
    #[serde(default, rename = "Operator")]
    operator: Vec<OperatorBody>,
}

#[derive(Debug, Deserialize)]
struct OperatorBody {
    #[serde(default)]
    key: Option<String>,
    #[serde(default)]
    title: Option<String>,
}

impl From<MetaInner> for Vocabulary {
    fn from(inner: MetaInner) -> Self {
        Self {
            types: inner
                .types
                .into_iter()
                .filter_map(|body| {
                    let raw_type = body.kind?;
                    Some(FilteringType {
                        libtype: ItemKind::from_plex(&raw_type),
                        raw_type,
                        filters: body
                            .filter
                            .into_iter()
                            .filter_map(|filter| {
                                Some(DiscoveredFilter {
                                    filter: filter.filter?,
                                    filter_type: filter.filter_type.unwrap_or_default(),
                                    title: filter.title,
                                    key: filter.key.filter(|key| !key.is_empty()),
                                })
                            })
                            .collect(),
                        sorts: body
                            .sort
                            .into_iter()
                            .filter_map(|sort| {
                                Some(DiscoveredSort {
                                    key: sort.key?,
                                    title: sort.title,
                                    default_direction: sort.default_direction,
                                })
                            })
                            .collect(),
                        fields: body
                            .field
                            .into_iter()
                            .filter_map(|field| {
                                Some(DiscoveredField {
                                    key: field.key?,
                                    field_type: field.field_type.unwrap_or_default(),
                                    sub_type: field.sub_type,
                                    title: field.title,
                                })
                            })
                            .collect(),
                    })
                })
                .collect(),
            field_types: inner
                .field_types
                .into_iter()
                .filter_map(|body| {
                    Some(FieldType {
                        field_type: body.field_type?,
                        operators: body
                            .operator
                            .into_iter()
                            .filter_map(|operator| {
                                Some(FieldOperator {
                                    key: operator.key?,
                                    title: operator.title,
                                })
                            })
                            .collect(),
                    })
                })
                .collect(),
        }
    }
}

impl PlexServerClient {
    /// Reads one library's declared filter vocabulary.
    ///
    /// Asked with a zero-size window, because the vocabulary rides alongside a
    /// result set and the result set is not wanted: fetching a library's worth
    /// of items to read its field list would put `I-PERF-1` at the mercy of a
    /// discovery pass.
    ///
    /// # Errors
    /// Returns [`ServerError::Transport`] when the server did not answer, and
    /// [`ServerError::Incomplete`] when it answered without a `Meta` block —
    /// which is a server that did not describe itself, never a server with no
    /// filters (P1).
    #[tracing::instrument(skip(self))]
    pub async fn vocabulary(
        &self,
        section: &SectionKey,
        libtype: ItemKind,
    ) -> Result<Vocabulary, ServerError> {
        let query = ItemQuery::new(Window::first(0))
            .of_type(libtype)
            .including_meta();
        let url = self.endpoint(&format!("library/sections/{section}/all"), &query.pairs())?;
        let body: MetaBody = self.container(Method::GET, &url, None).await?;
        body.meta
            .map(Vocabulary::from)
            .ok_or(ServerError::Incomplete {
                call: "GET /library/sections/{key}/all?includeMeta=1",
                missing: "the Meta block describing its own filters",
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"{
      "Meta": {
        "Type": [
          {"type":"movie",
           "Filter":[
             {"filter":"genre","filterType":"string","title":"Genre",
              "key":"/library/sections/1/genre?type=1"},
             {"filter":"year","filterType":"integer","title":"Year"}],
           "Sort":[{"key":"titleSort","title":"Title","defaultDirection":"asc"}],
           "Field":[
             {"key":"audioLanguage","type":"string","title":"Audio Language"},
             {"key":"userRating","type":"integer","subType":"rating","title":"Rating"}]}
        ],
        "FieldType": [
          {"type":"string","Operator":[{"key":"=","title":"is"},{"key":"!=","title":"is not"}]},
          {"type":"integer","Operator":[{"key":">>=","title":"is at least"}]}
        ]
      }
    }"#;

    fn vocabulary() -> Vocabulary {
        let body: MetaBody = serde_json::from_str(FIXTURE).expect("parses");
        Vocabulary::from(body.meta.expect("the fixture describes itself"))
    }

    #[test]
    fn a_filtering_type_carries_its_filters_sorts_and_fields() {
        let vocabulary = vocabulary();
        let movies = &vocabulary.types[0];
        assert_eq!(movies.libtype, Some(ItemKind::Movie));
        assert_eq!(movies.filters.len(), 2);
        assert_eq!(movies.sorts[0].key, "titleSort");
        assert_eq!(movies.fields[1].sub_type.as_deref(), Some("rating"));
    }

    #[test]
    fn a_filter_with_choices_carries_the_endpoint_that_lists_them() {
        let vocabulary = vocabulary();
        assert_eq!(
            vocabulary.types[0].filters[0].key.as_deref(),
            Some("/library/sections/1/genre?type=1")
        );
        // A free-value filter has no choice endpoint, and an empty string is
        // not one: a request to `""` resolves to the server root.
        assert_eq!(vocabulary.types[0].filters[1].key, None);
    }

    #[test]
    fn the_operator_table_answers_by_field_type() {
        let vocabulary = vocabulary();
        let string_operators = vocabulary
            .operators_for("string")
            .expect("the server described strings");
        assert_eq!(string_operators.len(), 2);
        assert_eq!(string_operators[0].key, "=");
        assert_eq!(
            vocabulary
                .operators_for("integer")
                .expect("the server described integers")[0]
                .key,
            ">>="
        );
    }

    #[test]
    fn a_type_the_server_did_not_describe_is_unknown_and_not_unfilterable() {
        // The difference between "fall back to local evaluation" and "this
        // predicate can never match" (PRD §13.2.4).
        assert_eq!(vocabulary().operators_for("duration"), None);
    }

    #[test]
    fn a_library_type_this_build_does_not_model_keeps_its_name() {
        let body: MetaBody =
            serde_json::from_str(r#"{"Meta":{"Type":[{"type":"track"}]}}"#).expect("parses");
        let vocabulary = Vocabulary::from(body.meta.expect("described"));
        assert_eq!(vocabulary.types[0].libtype, None);
        assert_eq!(vocabulary.types[0].raw_type, "track");
    }

    #[test]
    fn an_answer_with_no_meta_block_is_absent_rather_than_an_empty_vocabulary() {
        let body: MetaBody = serde_json::from_str(r#"{"size":0}"#).expect("parses");
        assert!(body.meta.is_none());
    }
}

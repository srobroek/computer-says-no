# Spec Quality Checklist: MLP Multi-Category Classification

**Purpose**: Validate requirement completeness, clarity, and consistency before planning
**Created**: 2026-04-02
**Feature**: [spec.md](../spec.md)

## Requirement Completeness

- [x] CHK001 - Is the minimum number of phrases *per category* specified, or only the 4-phrase total minimum? → Fixed: FR-002 now requires at least 2 phrases per category AND 4 total. [Spec §FR-002]
- [x] CHK002 - Are training hyperparameters specified for the multi-category path? → Fixed: FR-002 explicitly states reuse of binary defaults. [Spec §FR-002]
- [x] CHK003 - Is the deterministic category ordering defined? → Fixed: FR-011 specifies alphabetical ordering for input features and softmax output indices. [Spec §FR-011]
- [x] CHK004 - Are requirements defined for what `list_sets` reports beyond category names? → Fixed: FR-012 now includes per-category phrase counts. [Spec §FR-012]
- [x] CHK005 - Is the sarcasm curation decision process documented? → Fixed: FR-006 defines curation criteria (frustration=emotional, correction=directive, sarcasm=evaluate per-phrase). [Spec §FR-006]

## Requirement Clarity

- [x] CHK006 - Is "sufficient training data" quantified beyond "at least 4 total phrases"? → Fixed: FR-002 specifies 2 per category AND 4 total. [Spec §FR-002]
- [x] CHK007 - Is "category-tailored prompt" defined with specific guidance? → Fixed: FR-009 provides prompt guidance per category. [Spec §FR-009]
- [x] CHK008 - Is the cosine feature computation for multi-category specified unambiguously? → Fixed: FR-011 defines per-category features as [max_similarity, mean_similarity, margin_vs_next_best]. [Spec §FR-011]

## Requirement Consistency

- [x] CHK009 - Are FR-006 and edge case 1 ("only 2 categories") consistent? → Fixed: edge case clarified that mode is determined by TOML field, not category count. [Spec §Edge Cases]
- [x] CHK010 - Is `MultiCategoryResult` consistent with existing struct in classifier.rs? → Fixed: FR-008 and Key Entities explicitly state reuse. [Spec §FR-008]
- [x] CHK011 - Does FR-003 conflict with shared code path changes? → Fixed: FR-003 clarifies shared paths are extended via branching, not modified in-place. [Spec §FR-003]

## Acceptance Criteria Quality

- [x] CHK012 - Is accuracy the right metric given class imbalance? → Fixed: SC-001 changed to macro-averaged F1 >= 80%. [Spec §SC-001]
- [x] CHK013 - Is "within 2x binary latency" measurable without an absolute baseline? → Fixed: SC-002 adds <15ms absolute target (binary baseline ~5ms). [Spec §SC-002]
- [x] CHK014 - Is SC-004 meaningful before phrase redistribution? → Fixed: SC-004 reworded to reference "representative sample post-curation". [Spec §SC-004]

## Scenario Coverage

- [x] CHK015 - Are daemon protocol requirements validated? → Fixed: assumption validated with code reference (serde serialization of ClassifyResult). Added edge case. [Spec §Assumptions, §Edge Cases]
- [x] CHK016 - Are binary→multi-category cache transition requirements defined? → Fixed: edge case explains separate hash prefixes ("v2-char256" vs "v3-multicat"), old binary weights unaffected. [Spec §Edge Cases, §FR-004]
- [x] CHK017 - Are user-created multi-category sets eligible for MLP? → Fixed: FR-013 added — any multi-category set meeting FR-002 minimums gets MLP training. [Spec §FR-013]

## Edge Case Coverage

- [x] CHK018 - Is behavior defined when two categories tie? → Fixed: alphabetically-first category wins. [Spec §Edge Cases]
- [x] CHK019 - Is behavior defined for zero phrases in a category? → Fixed: existing loader validation rejects it (error, not skip). [Spec §Edge Cases]
- [x] CHK020 - Are content hash versioning requirements defined? → Fixed: FR-004 specifies "v3-multicat" prefix, explicit no-collision with binary "v2-char256". [Spec §FR-004]

## Dependencies & Assumptions

- [x] CHK021 - Is data structure reuse validated against MLP needs? → Fixed: assumption expanded to explain that existing structures provide exactly what MLP needs. [Spec §Assumptions]
- [x] CHK022 - Is benchmark harness deferral tracked? → Fixed: assumption clarifies it's tracked as a separate issue, not a spec. [Spec §Assumptions]

## Notes

- All 22 items resolved. Spec updated in place.
- No items deferred — all addressed before planning.

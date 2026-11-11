# Upstream draft: child name patterns match attribute nodes

Not filed. Patch: [xrust-pattern-node-types.patch](xrust-pattern-node-types.patch),
after patch 11. The public `built_in_node_types` regression still failed after adding
missing built-in rules: expected `<out>ATB</out>`, actual `<out>TB</out>`.
The built-in `child::*` rule incorrectly matched the attribute, then visited its empty
child list. An explicit `match="*"` has the same underlying problem; the dedicated
`xslt_child_name_patterns_do_not_match_attributes` regression covers that case.

[XSLT 1.0 §5.2](https://www.w3.org/TR/1999/REC-xslt-19991116#patterns): child-axis
name patterns match elements, while attribute-axis patterns match attributes. The
SelfAxis reverse matcher called only the name test, accepting any named node.
Require an element for a child name test, and exclude attributes and namespaces from
child kind tests. Leave SelfAttribute unchanged. No public API change.
Check GitLab main before submission.

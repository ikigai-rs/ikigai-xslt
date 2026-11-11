# Upstream draft: equal-priority template conflicts select the first declaration

Not filed. Patch: [xrust-template-ties.patch](xrust-template-ties.patch), after patch 9.
The public `template_priorities_and_ties` test reproduced `<out>first</out>` where
`<out>last</out>` is required by the chosen recovery rule, using two `match="item"`
templates with equal priority. `xslt_equal_priority_uses_last_declaration` also covers
the initial root dispatch, separately from `apply-templates`.

[XSLT 1.0 §5.5](https://www.w3.org/TR/1999/REC-xslt-19991116#conflict) permits an
error or recovery by choosing the last declaration at equal import precedence and
priority. The engine silently chose the first. Two causes: the compiler set every
matched template's document_order to None, and Template::cmp returned Less for equal
numeric priorities. Record declaration order and compare import depth for equal priorities (returning
Equal only at equal depth),
so the existing conflict-resolution code can choose the last candidate. No API change.
Check GitLab main before submitting.

The full workspace run also exercises the existing `tr_apply_templates_import`
regression. It exposed that simply returning Equal could reorder templates at
different import depths; the comparator retains the engine's shallower-import-first
ordering on equal numeric priorities.

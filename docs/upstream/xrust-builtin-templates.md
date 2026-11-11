# Upstream draft: missing built-in templates in modes and for leaf nodes

Not filed. Patch: [xrust-builtin-templates.patch](xrust-builtin-templates.patch), after
patch 10. Reproduced through the public `built_in_templates_keep_the_mode` and
`built_in_node_types` audit tests before changing the engine. The upstream regression
`xslt_builtin_templates_cover_modes_and_node_types` also checks an undeclared mode. The upstream patch isolates those missing rules;
the carried regression additionally selects an attribute, which also needs patch 12's
child-pattern node-type fix.

Given `<doc><item>A</item><!--ignored--><?pi ignored?></doc>`, applying templates to
`doc` in mode `m`, with an explicit `item` template in mode `m` returning M, must
produce M. The engine instead errors: no matching template for doc in mode m.
Processing comments or processing instructions without explicit rules likewise errors;
attributes lack the rule that copies their string value as text.

[XSLT 1.0 §5.8](https://www.w3.org/TR/1999/REC-xslt-19991116#built-in-rule) defines
built-in rules in every mode. When explicit/registered candidates are absent, supply
an applicable built-in rule: recurse on root/element children in the same mode, copy
text/attribute string values as text, and emit nothing for comments, processing
instructions and namespace nodes. The rule is used only as a fallback, so explicit
templates always win. No public API change. Check GitLab main before submission.

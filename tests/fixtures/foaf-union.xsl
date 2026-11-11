<?xml version="1.0" encoding="UTF-8"?>
<!-- The shape of services/edge/config/foaf.xsl: a six-way union of prefixed names under a mode
     (ledger #3396). Prefixes are bound on xsl:stylesheet only. -->
<xsl:stylesheet version="1.0"
    xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
    xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
    xmlns:foaf="http://xmlns.com/foaf/0.1/">
  <xsl:output method="text"/>

  <xsl:template match="/">
    <xsl:apply-templates select="rdf:RDF/foaf:Person/*" mode="link"/>
  </xsl:template>

  <xsl:template match="foaf:homepage | foaf:weblog | foaf:workplaceHomepage | foaf:schoolHomepage | foaf:openid | foaf:page" mode="link">
    <xsl:value-of select="local-name()"/>
    <xsl:text>=</xsl:text>
    <xsl:value-of select="@rdf:resource"/>
    <xsl:text>;</xsl:text>
  </xsl:template>

  <xsl:template match="*" mode="link"/>
</xsl:stylesheet>

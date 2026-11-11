<?xml version="1.0" encoding="UTF-8"?>
<!-- The parameter shape of services/edge/config/foaf.xsl (ledger #3398), not the file itself:
     two templates each declare <xsl:param name="iri"/>, one forwards $iri to the other, and
     `last-segment` recurses on substring-after($iri, '/'). Six sections, each with an eyebrow. -->
<xsl:stylesheet version="1.0"
    xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
    xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
    xmlns:foaf="http://xmlns.com/foaf/0.1/">
  <xsl:output method="html"/>

  <xsl:template match="/">
    <main id="main">
      <xsl:apply-templates select="rdf:RDF/foaf:Person"/>
    </main>
  </xsl:template>

  <xsl:template match="foaf:Person">
    <section>
      <p class="eyebrow">Person</p>
      <h1><xsl:value-of select="foaf:name"/></h1>
      <p>
        <xsl:call-template name="last-segment">
          <xsl:with-param name="iri" select="string(@rdf:about)"/>
        </xsl:call-template>
      </p>
    </section>
    <section>
      <p class="eyebrow">Links</p>
      <ul><xsl:apply-templates select="foaf:homepage | foaf:weblog | foaf:workplaceHomepage | foaf:schoolHomepage | foaf:openid | foaf:page" mode="link"/></ul>
    </section>
    <section>
      <p class="eyebrow">Knows</p>
      <ul><xsl:apply-templates select="foaf:knows" mode="link"/></ul>
    </section>
    <section>
      <p class="eyebrow">Interests</p>
      <ul><xsl:apply-templates select="foaf:interest" mode="link"/></ul>
    </section>
    <section>
      <p class="eyebrow">Accounts</p>
      <ul><xsl:apply-templates select="foaf:account" mode="link"/></ul>
    </section>
    <section>
      <p class="eyebrow">Projects</p>
      <ul><xsl:apply-templates select="foaf:currentProject" mode="link"/></ul>
    </section>
  </xsl:template>

  <xsl:template match="*" mode="link">
    <li>
      <xsl:call-template name="link-label">
        <xsl:with-param name="iri" select="string(@rdf:resource | */@rdf:about)"/>
      </xsl:call-template>
    </li>
  </xsl:template>

  <xsl:template name="link-label">
    <xsl:param name="iri"/>
    <a href="{$iri}">
      <xsl:call-template name="last-segment">
        <xsl:with-param name="iri" select="$iri"/>
      </xsl:call-template>
    </a>
  </xsl:template>

  <xsl:template name="last-segment">
    <xsl:param name="iri"/>
    <xsl:choose>
      <xsl:when test="contains($iri, '/')">
        <xsl:call-template name="last-segment">
          <xsl:with-param name="iri" select="substring-after($iri, '/')"/>
        </xsl:call-template>
      </xsl:when>
      <xsl:otherwise>
        <xsl:value-of select="$iri"/>
      </xsl:otherwise>
    </xsl:choose>
  </xsl:template>
</xsl:stylesheet>

<xsl:stylesheet version="1.0" xmlns:xsl="http://www.w3.org/1999/XSL/Transform">
  <xsl:output method="xml"/>
  <xsl:template match="/">
    <items>
      <xsl:for-each select="/rss/channel/item">
        <item>
          <xsl:if test="title"><title><xsl:value-of select="title"/></title></xsl:if>
          <xsl:if test="link"><link><xsl:value-of select="link"/></link></xsl:if>
          <xsl:call-template name="date"/>
        </item>
      </xsl:for-each>
    </items>
  </xsl:template>
  <xsl:template name="date">
    <date><xsl:value-of select="substring(pubDate, 1, 16)"/></date>
  </xsl:template>
</xsl:stylesheet>

<xsl:stylesheet version="1.0"
  xmlns:xsl="http://www.w3.org/1999/XSL/Transform"
  xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
  xmlns:rss="http://purl.org/rss/1.0/"
  xmlns:dc="http://purl.org/dc/elements/1.1/"
  xmlns:dcterms="http://purl.org/dc/terms/"
  xmlns:content="http://purl.org/rss/1.0/modules/content/"
  xmlns:atom="http://www.w3.org/2005/Atom"
  exclude-result-prefixes="atom">
<!-- Fixture source: https://github.com/ikigai-rs/ikigai-tutorial/blob/225e6c3611517a42f6171e9811b0a54caff4caf0/examples/rss/ws/items.xsl -->
  <!-- RSS 2.0 in, RDF/XML out, in RSS 1.0's own vocabulary (RSS 1.0 was RDF) plus Dublin Core.
       One IRI per item: its guid when that is a permalink URL, else its link. No blank nodes. -->
  <xsl:output method="xml"/>
  <!-- ⚠ xrust 2.3 lowercases the LITERAL value of a prefixed attribute (rdf:datatype="…XMLSchema#dateTime"
       becomes "…xmlschema#datetime"), and the names inside an attribute value template with it. A value
       computed from data keeps its case, so the datatype IRI lives in an all-lowercase-named variable. -->
  <xsl:variable name="xsddatetime" select="'http://www.w3.org/2001/XMLSchema#dateTime'"/>

  <xsl:template match="/">
    <xsl:variable name="self" select="/rss/channel/atom:link[@rel='self']/@href"/>
    <xsl:variable name="feed">
      <xsl:choose>
        <xsl:when test="$self"><xsl:value-of select="$self"/></xsl:when>
        <xsl:otherwise><xsl:value-of select="/rss/channel/link[1]"/></xsl:otherwise>
      </xsl:choose>
    </xsl:variable>
    <rdf:RDF>
      <rss:channel rdf:about="{$feed}">
        <rss:title><xsl:value-of select="/rss/channel/title[1]"/></rss:title>
        <rss:link><xsl:value-of select="/rss/channel/link[1]"/></rss:link>
        <xsl:if test="/rss/channel/ttl">
          <dcterms:valid><xsl:value-of select="/rss/channel/ttl[1]"/></dcterms:valid>
        </xsl:if>
      </rss:channel>
      <xsl:for-each select="/rss/channel/item">
        <xsl:variable name="guid" select="normalize-space(guid[1])"/>
        <xsl:variable name="iri">
          <xsl:choose>
            <xsl:when test="guid and not(guid[1]/@isPermaLink = 'false') and starts-with($guid, 'http')">
              <xsl:value-of select="$guid"/>
            </xsl:when>
            <xsl:otherwise><xsl:value-of select="normalize-space(link[1])"/></xsl:otherwise>
          </xsl:choose>
        </xsl:variable>
        <rss:item rdf:about="{$iri}">
          <dcterms:isPartOf rdf:resource="{$feed}"/>
          <xsl:if test="title"><rss:title><xsl:value-of select="title[1]"/></rss:title></xsl:if>
          <rss:link><xsl:value-of select="normalize-space(link[1])"/></rss:link>
          <xsl:if test="guid"><dc:identifier><xsl:value-of select="$guid"/></dc:identifier></xsl:if>
          <xsl:if test="description"><rss:description><xsl:value-of select="description[1]"/></rss:description></xsl:if>
          <xsl:if test="content:encoded"><content:encoded><xsl:value-of select="content:encoded[1]"/></content:encoded></xsl:if>
          <xsl:for-each select="category"><dc:subject><xsl:value-of select="normalize-space(.)"/></dc:subject></xsl:for-each>
          <xsl:if test="author"><dc:creator><xsl:value-of select="author[1]"/></dc:creator></xsl:if>
          <xsl:if test="dc:creator"><dc:creator><xsl:value-of select="dc:creator[1]"/></dc:creator></xsl:if>
          <xsl:if test="pubDate">
            <dcterms:issued><xsl:value-of select="normalize-space(pubDate[1])"/></dcterms:issued>
            <xsl:call-template name="w3cdtf"><xsl:with-param name="d" select="normalize-space(pubDate[1])"/></xsl:call-template>
          </xsl:if>
        </rss:item>
      </xsl:for-each>
    </rdf:RDF>
  </xsl:template>

  <!-- ⚠ xrust 2.3: arithmetic on string-length() is NaN unless it is wrapped in number(), so every one is. -->
  <!-- RFC 822 "Wed, 07 Oct 2026 10:00:00 -0700" (or GMT/UT/Z, a one-digit day, no seconds) to an
       xsd:dateTime "2026-10-07T10:00:00-07:00". Anything else: no dc:date, the raw dcterms:issued stays. -->
  <xsl:template name="w3cdtf">
    <xsl:param name="d"/>
    <xsl:variable name="r">
      <xsl:choose>
        <xsl:when test="contains($d, ',')"><xsl:value-of select="normalize-space(substring-after($d, ','))"/></xsl:when>
        <xsl:otherwise><xsl:value-of select="$d"/></xsl:otherwise>
      </xsl:choose>
    </xsl:variable>
    <xsl:variable name="day" select="substring-before($r, ' ')"/>
    <xsl:variable name="r2" select="substring-after($r, ' ')"/>
    <xsl:variable name="mon" select="substring(substring-before($r2, ' '), 1, 3)"/>
    <xsl:variable name="r3" select="substring-after($r2, ' ')"/>
    <xsl:variable name="year" select="substring-before($r3, ' ')"/>
    <xsl:variable name="r4" select="substring-after($r3, ' ')"/>
    <xsl:variable name="time" select="substring-before($r4, ' ')"/>
    <xsl:variable name="zone" select="substring-after($r4, ' ')"/>
    <xsl:variable name="m" select="number(string-length(substring-before('JanFebMarAprMayJunJulAugSepOctNovDec', $mon))) div 3 + 1"/>
    <xsl:variable name="mm" select="substring(concat('0', $m), number(string-length(concat('0', $m))) - 1, 2)"/>
    <xsl:variable name="dd" select="substring(concat('0', $day), number(string-length(concat('0', $day))) - 1, 2)"/>
    <xsl:variable name="hms">
      <xsl:choose>
        <xsl:when test="string-length($time) = 5"><xsl:value-of select="concat($time, ':00')"/></xsl:when>
        <xsl:otherwise><xsl:value-of select="$time"/></xsl:otherwise>
      </xsl:choose>
    </xsl:variable>
    <xsl:variable name="tz">
      <xsl:choose>
        <xsl:when test="$zone = 'GMT' or $zone = 'UT' or $zone = 'UTC' or $zone = 'Z'">Z</xsl:when>
        <xsl:when test="string-length($zone) = 5 and (starts-with($zone, '+') or starts-with($zone, '-'))">
          <xsl:value-of select="concat(substring($zone, 1, 3), ':', substring($zone, 4, 2))"/>
        </xsl:when>
        <xsl:when test="string-length($zone) = 6 and substring($zone, 4, 1) = ':'"><xsl:value-of select="$zone"/></xsl:when>
      </xsl:choose>
    </xsl:variable>
    <xsl:if test="string-length($year) = 4 and contains('JanFebMarAprMayJunJulAugSepOctNovDec', $mon) and string-length($mon) = 3 and string-length($hms) = 8 and string-length($tz) &gt; 0">
      <dc:date rdf:datatype="{$xsddatetime}"><xsl:value-of select="concat($year, '-', $mm, '-', $dd, 'T', $hms, $tz)"/></dc:date>
    </xsl:if>
  </xsl:template>
</xsl:stylesheet>

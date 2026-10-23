<!-- Generated from SECURITY.nepld; renderer nepl3-tools.markdown-annotated-pages/4; page security; source SHA-256 f6235a9521fabfa0a8437912ca9d85b030f33d47f0cc2c4f6e1fe5f06afe3ce4; alias input SHA-256 eb097a3049bf6401d9b0b30fe615fdf110f1be5d9e481a9c4868bf879887dacb; page input SHA-256 a59b897b79856e8ccf32beccc5b581066fe2a5ee9d5f3f458412d3e97f30a3f8. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="セキュリティ"></a>

# セキュリティ

[正本（NEPL3d）](<SECURITY.nepld>)

<ruby>現在<rt>げんざい</rt></ruby>は<ruby>共通<rt>きょうつう</rt></ruby>runtimeを<ruby>実装<rt>じっそう</rt></ruby>・<ruby>検証<rt>けんしょう</rt></ruby>している<ruby>段階<rt>だんかい</rt></ruby>で、サポート<ruby>対象<rt>たいしょう</rt></ruby>のruntime releaseはありません。セキュリティ<ruby>修正<rt>しゅうせい</rt></ruby>はmainへ<ruby>反映<rt>はんえい</rt></ruby>します。

<ruby>未公開<rt>みこうかい</rt></ruby>の<ruby>脆弱性<rt>ぜいじゃくせい</rt></ruby>は [GitHubの<ruby>非公開報告<rt>ひこうかいほうこく</rt></ruby>](<https\:\/\/github\.com\/neknaj\/NEPL3\/security\/advisories\/new>) を<ruby>利用<rt>りよう</rt></ruby>してください。<ruby>再現入力<rt>さいげんにゅうりょく</rt></ruby>、<ruby>対象<rt>たいしょう</rt></ruby>commit、<ruby>影響<rt>えいきょう</rt></ruby>、<ruby>実行環境<rt>じっこうかんきょう</rt></ruby>を<ruby>添<rt>そ</rt></ruby>えてください。<ruby>秘密情報<rt>ひみつじょうほう</rt></ruby>や<ruby>悪用可能<rt>あくようかのう</rt></ruby>な<ruby>詳細<rt>しょうさい</rt></ruby>を<ruby>公開<rt>こうかい</rt></ruby>Issueへ<ruby>記載<rt>きさい</rt></ruby>しないでください。

<ruby>将来<rt>しょうらい</rt></ruby>の<ruby>外部<rt>がいぶ</rt></ruby>provider、<ruby>文法<rt>ぶんぽう</rt></ruby>package、NDF<ruby>入力<rt>にゅうりょく</rt></ruby>の<ruby>信頼境界<rt>しんらいきょうかい</rt></ruby>は [エディタ<ruby>仕様<rt>しよう</rt></ruby>](<doc\/spec\/08\-editor\.md>) と [<ruby>交換仕様<rt>こうかんしよう</rt></ruby>](<doc\/spec\/09\-portability\.md>) に<ruby>従<rt>したが</rt></ruby>います。<ruby>設計上<rt>せっけいじょう</rt></ruby>の<ruby>要件<rt>ようけん</rt></ruby>と<ruby>実装済<rt>じっそうず</rt></ruby>みの<ruby>防御<rt>ぼうぎょ</rt></ruby>は<ruby>区別<rt>くべつ</rt></ruby>し、<ruby>状態<rt>じょうたい</rt></ruby>を [implementation\-status\.json](<implementation\-status\.json>) に<ruby>記録<rt>きろく</rt></ruby>します。

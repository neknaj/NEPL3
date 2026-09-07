# 19. HTML fragmentの構造契約

`interfaces/markup.json` を言語中立の値schema、`nepl3-markup` をnativeの検査・serializerとする。現段階の対象はHTML fragmentである。MathML/SVG/KaTeX専用profile、DocumentShell、assetのbyte列・MIME・license検査、Doc PreparedArticle、完全な操作Reportは後続実装であり、既存の最終要件から除かない。

HtmlFragmentはroot U64と平坦なHtmlNode列を持つ。Textまたは固定HtmlTag・型付きattribute列・子U64列だけを許す。Rustのenum ordinalをwireへ保存しない。未知tag/attribute、RawHtml、style、event handler、script、任意namespaceを成功として受け入れない。木の深さは参照によって表し、深い入力のdropにRust call stackを使わない。

rootのBlock slotはHTML flow要素またはText、Phrasing slotはphrasing要素またはTextを取る。P/span/見出し/em/strong/pre/code/aはphrasing子だけを持つ。aはphrasing子孫を通しても入れ子にできない。ul/olの直接子はli、tableはcaption最大一つ・thead最大一つ・tbody列の順、thead/tbodyはtr、trはth/tdである。tbodyを省略してHTML parserの自動挿入へ依存しない。caption内のtable、th内のsectioning/heading contentを子孫まで拒否する。figureのfigcaptionは最大一つで先頭または末尾。br/imgには子を置かない。imgはsrcとaltを必須にし、空altは装飾画像として表現できる。

すべての参照を検査し、cycle・未到達nodeを拒否する。DAG共有は各表示出現を展開して検査し、同じnodeを二度使って同じidが二度出力される場合もDuplicateIdにする。id/data-nepl-id/data-nepl-groupはASCII `[a-z][a-z0-9-]*`。Fragment hrefは同じfragmentの実idへ到達する。attributeの名前重複を拒否する。

classはHtmlPolicyに列挙されたbackend登録名だけを許す。名前はidと同じ字句制約で、attribute内の空列・重複を拒否する。受信したHtmlPolicyの自己申告はstylesheetの信頼・実在・内容検査の証明ではない。実hostは独立した検査済みbackend資源と照合する。Langは共通RFC 5646字句検査を再利用する。roleはheading/img/group/note、aria-levelは正整数。画像width/heightは正整数、ol.startは0以上の符号付き32bit上限以内、th.scopeはrow/colだけ。

リンクはFragment・Artifact・Externalを分ける。Artifactと画像srcのpathは空でない相対segment列で、各segmentはASCII英数字と`-_.`、空segment/`.`/`..`を禁止する。これはhostが割り当てる配布pathであり、任意の著者pathをそのまま通す入口ではない。page/asset解決が非ASCII等の著者IDから配布pathへ変換し、その対応とbyte列を別途検査する。

Externalは現在のconstrained URI profileでは小文字http/https/mailto scheme、ASCII、正しいpercent escapeのみ。http(s)のauthorityはASCII DNS/punycodeまたはIPv4表記のhostと省略可能なASCII数字列のu16 port（符号なし）、userinfoなし。空白・backslash・不正percent・network-path referenceを拒否する。これは全URL標準のparserではない。より広い正当URIの扱いはDoc preparation時の正規化またはprofile拡張として明示する。リンクの存在、アクセス可否、ページrevisionを構造検査から推測しない。

serializerは検査proofを借用し、attributeをASCII名前順に出力する。13章のXML文字・escapeを保ち、属性は二重引用符で囲み、void要素に終端tagを付けない。HTML parserがpre開始直後のLFを一つ除去する規則に合わせ、pre開始tag直後へ固定LFを一つ追加して元本文の先頭改行を保持する。doctype/head/CSP/CSSはDocumentShellの別責務である。

Budgetは全ての処理で同じものを使い、展開した表示出現、callerを含む深さ、仕事量、割当、出力を課金する。Textのescapeはsemantic値を受けるbyte処理なのでSourceBytes/Nodes/Depthの入場を代行しない。OutputBytesは各生成片の生成許可量であり、後段の割当停止でも先に課金した値を巻き戻さない。返却できる完全な文字列だけを成功にする。論理AllocationUnitsは物理allocatorやOOMの完全な捕捉ではない。

NDFのHtmlRequest受信はschema検査と同じnative構造検査を実行する。ポインタ、送信processの登録表、以前のproofを受信時の根拠にしない。ただしHtmlPolicyの信頼やasset availabilityを確定する操作は未完であり、この値boundaryを完成したrender/provider操作として広告しない。

内容モデルは[HTML Standardのtable](https://html.spec.whatwg.org/multipage/tables.html#the-table-element)、[a](https://html.spec.whatwg.org/multipage/text-level-semantics.html#the-a-element)、[URL Standard](https://url.spec.whatwg.org/)と照合する。受入時には実HTML parser/ブラウザでserializer結果の構造も照合し、文字列一致だけで表示の適合を推定しない。

RubyはHTML Standardのbase/annotation群を検査する。baseはRuby子孫を含まないphrasing列、またはRuby子孫を持たない単一Ruby要素。各baseにはrt列、あるいはrp開始・rt/rp交互列を必須にし、空Rubyやbaseだけを拒否する。rpの子はTextだけで、annotationの直前・直後に置く。表の全行のcell数は同一とする。ゼロ列はDoc意味モデルに従い保存できる。これらは[HTML Ruby内容モデル](https://html.spec.whatwg.org/multipage/text-level-semantics.html#the-ruby-element)に従い、ブラウザが不正markupを補正して表示できることを合法性の根拠にしない。

入力長に比例するstate初期化や子enqueueに先立ってWorkを課金する。小さいWork上限で大量の処理待ちを確保し終えてから停止することを避け、同じ停止理由と単調なUsageを維持する。

fragmentの内容モデルproofは、任意深度のbrowser DOM保持を保証しない。独立した実Chromium/Firefoxの検査で、Div鎖の深さ512/600は最大511へ平坦化された。H1の文書shellとpreviewでは包囲要素を含む出力深度profileを検査し、超過を黙って平坦化せず理由付きで扱う。coreの深い意味構造・反復処理・dropの要件と、各出力backend/閲覧環境の制約を分離する。

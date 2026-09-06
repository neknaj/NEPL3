# 13. 再現性・schema識別・契約の判定

## 方針

共有する意味と実装固有のアルゴリズムを分ける。digestの自己参照、言語同士の相互埋め込みによるhash循環、source順と識別子順の混同を避ける。

## 1. digest

Bytes32のdigest関数はSHA-256に固定する。sourceとresourceのcontent digestは元のbyte列にそのまま適用する。BOM、改行、空白も内容の一部であり、Unicode正規化を行わない。SourceContentのURIはhostが付ける論理的な絶対locatorであり、SourceRefの同一性とは別に保持する。coreはOS path canonicalizationをしない。

このlocatorの字句profileはASCIIのscheme `[A-Za-z][A-Za-z0-9+.-]*:` と空でない後続文字列である。control・未escapeの空白を拒否し、`%` は2桁のASCII hexが続く場合だけ許す。scheme以降のUnicode文字を正規化しない。相対path・空文字をabsolute locatorとして受け付けない。これはhost内部locatorの境界であり、HTTPのauthorityなどscheme固有の妥当性や到達可能性を保証しない。外部URLとして使用するadapterはその用途のURI/IRI検査を行う。schemeの根拠は [RFC 3986 §3.1](https://www.rfc-editor.org/rfc/rfc3986.html#section-3.1)、Unicodeを含む識別子とURIの区別は [RFC 3987](https://www.rfc-editor.org/rfc/rfc3987.html)。

SchemaRef.digestは、schemaの正規descriptorを対象とする。正規descriptorはpackage名、revision、kind/variant/field定義、意味上のoperation署名、局所制約識別を含み、source位置、documentation、cache、生成時刻を含まない。自己参照とforeign schemaは(package, revision, typeName)の記号的参照とし、このdescriptor内へ相互のdigestを再帰的に埋め込まない。実際に使用するforeign digestはProfileの解決済み一覧で固定して検査する。これによりDocとMathが互いの型を参照してもhashの固定点を計算する必要はない。

型のconstraintsは実行順ではなく、名前で識別する局所制約の集合である。空・重複の識別子を拒否し、canonical descriptorへ変換するときにUnicode scalar順のarrayへ並べる。fieldやordered-choiceの意味順を持つarrayとは区別する。

descriptorの正規化データはNull/Bool/非負Integer/Text/Array/Objectだけを使う。JSONをdigest入力として使う場合、object keyをUnicode scalar順にsortし、array順を維持し、空白なし、UTF-8、引用符とbackslashをescapeし、U+0000..001Fは小文字hexの\u00xxでescapeし、それ以外のscalarは直接UTF-8とする。整数は先頭zeroなし。float、負数、surrogate、重複keyは禁止。hash入力はASCII `NEPL3-SCHEMA-1` + 1byte zero + このcanonical JSON。

Grammar packageの意味digestはschema digestと、source位置を除いたreader/binding/style/shape/provider要件の正規descriptorに対して、同じ規則でdomain separatorを `NEPL3-PACKAGE-1` として計算する。局所kind IDはkind名のscalar順で割り当てる。field・宣言・ordered-choiceの意味上の順序はarrayとして保存する。名前で識別するsum/union/variantsのmap順には意味がなく、canonical JSONのkey sortで一致させる。variant payloadのfield順はarrayとして保存する。fieldの順序交換はdigestを変え、variant mapのkey順だけの交換は変えない。seedとcompile後のpackageは意味digestを比較し、provenanceの違いを意味不一致としない。source artifactのdigestは別に記録する。

wireの操作要求は、解決済みProfileの全SchemaRefとprovider revisionを含めてcache keyを作る。URIやsnapshot所属を除いてよいのは明示的な意味値だけのcacheであり、診断/editor結果のcacheにはsource identityとrevisionが必要。

## 2. native値とwire値

型名中のU64/Bytes32等の有限primitiveと、任意精度Natural/Integerを区別する。wire sourceはopaque SourceId/revision/digestを使う。同じIDをbundle内・操作間の対応づけに使用し、URIやnative allocation addressへ置き換えない。同じURI/revision/byte列を持つ独立文書もSourceIdが異なれば別snapshotである。r3のURI-based bijectionは、この場合にspec02の宣言同一性を失うためr4で訂正した。

native node indexはallocationごとのIDでもよい。wire bundleではrootからfield順に訪問した最初の出現順で連番にする。shared nodeは二回目以降referenceを使う。source tableはSourceIdのUnicode scalar順、revisionの数値順、digestのbyte順とし、schema tableはpackage/revision/digest順。Originの親参照はDAGを検査し、payload nodeとorigin nodeのID空間を分ける。SourceIdはこのnode index再採番の対象にしない。

NodeRefの訪問はdepth-firstで、Childをその位置、Childrenを列の順にたどる。rootは0となり、全NodeRefを書き換える。ForeignSyntaxはguest bundleで独立して再採番し、ForeignSyntax.rootもguestの0へ対応させる。wire bundleは単一rootの到達閉包を表すため、到達不能nodeはUnreachableNodeで拒否し、黙って破棄しない。native arenaの未使用slotは許せるが、出力対象bundleへ含めない。Missing/Unexpected/Unparsed等の回復構文もrootから参照して保持する。

TokenRef、Token内のViewRef、OriginRef、EnvironmentEntry.idはそれぞれの所有tableで宣言されたslotを指し、このNodeRef再採番の対象ではない。これらのtableとbinding/resource/role/relation/triviaの列順は値の一部として保存する。生成側はsource順または明示した生成順でtableを作り、allocation address・hash map列挙順を宣言順へ使わない。したがって、node arenaだけを並べ替えた同じ値はwire byte一致を要求するが、別tableの宣言順まで異なるgraphの同型性をこの規則だけで証明したとは扱わない。

SyntaxBundle.sourceMapsも宣言順の列として保存する。そのSpanはopaque source identityとbyte rangeを持ち、NodeRef再採番で変化しない。foreign bundleは独自のsourceMaps/sourcesを持つ。生成snapshotの予約identityもhostが固定してからnative/portable比較へ渡し、別実装が独自にIDを発明して同一byte列を偽装しない。

EnvironmentEntry.digestは `NEPL3-ENVIRONMENT-1` + zero byte + canonical NDF(Environment record) のSHA-256とする。entry自身のid/digestはhashへ入れない。bindings/resourcesの列順とbinding中のbundle局所OriginRefはEnvironment値の一部である。Origin tableを再編するhostは参照とdigestを共に更新し、別bundleへ同じ数値OriginRefだけを移して同一環境とみなさない。環境digest一致はoriginの実在・domain bindingの意味検査を代替しない。

この値は局所table参照を含む内容digestであり、参照先Originの閉包digestではない。別のOrigin tableで同じ番号を使えば同じ内容digestになり得る。reader・editor・診断のcache keyはEnvironmentEntry.digestだけでなく、ReaderContextのOrigin table・選択Profile・source bundleのidentityを固定する。出自を持つ結果を別bundleへ再利用しない。originの再採番時はbindingとEnvironmentEntry.digestに加え、ForeignSyntax.environment.digestを同時に更新する。

## 3. normal formとartifact

printの正規形は各言語のformal source表に従う。lossless再出力はoriginal SourceSnapshotの抽出であり、意味printerと同一視しない。sourceを持たない値にもprefix printerが使える。

HTML/XML出力はUTF-8、属性はnamespace URIとlocal nameのscalar順で出力する。モデル不変条件章の文字集合を検査後、Textと属性値の `&`、`<`、`>` を常に `&amp;`、`&lt;`、`&gt;` へescapeする。これによりXML文字データ中の `]]>` も直接出力されない。属性値は二重引用符で囲み、`"` は `&quot;` とする。TextのCRは `&#xD;`、属性値のTAB/LF/CRはそれぞれ `&#x9;`、`&#xA;`、`&#xD;` としてparse後の値を保持する。TextのTAB/LFはそのまま出す。HTMLのbrはvoid、XMLでは自己閉じを使う。外部から任意のnamespace URIを受け付けない。MathML/SVGのnamespaceはserializerが固定値を生成する。

pixelの完全一致はOS/font/browserに依存するため、このbackendの契約は安全なDOM内容・構造・宣言されたstyle/layout規則とする。同じbackend revisionの固定assetから生成するmarkup byte列は決定的。nativeとprocessの同実装経路ではbyte一致、異なる独立実装ではcanonical構造一致と定義済み表示規則を検査する。

## 4. limitsとproviderの互換

algorithmごとのwork消費量は異なってよい。十分な予算でCompleteになった同じ入力について、意味正規形・診断code/対象範囲・解決先が一致することを要求する。片方だけが予算不足の場合、値の不一致と同じ扱いをしない。ただしlimit違反の無視、partialをCompleteにすること、hostへ制御を返さないことは契約違反。

同じ入力・environment・source/resources・provider revision・budgetでの同じ実装の結果は決定的。外部clock、乱数、OS directory順などに意味を依存させない。

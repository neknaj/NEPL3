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

### Package意味正規形

実装の`CheckedLanguagePackage::semantic_json`はcategories、extensions、forms、leaves、modes、namespaces、payloadSchemas、reader、recovery、root、schemaのkeyを持つcanonical JSONを返す。Category/Mode/Namespace/Extensionは名前順、Formはcategoryとspelling順、Leafはcategoryとcanonical token-kind ID順、payloadSchemasはpackage/revision/digest順に並べる。重複するpayload SchemaRefは拒否する。各レコードのpayload field順は`interfaces/engine.json`の対応型に従う。ただしread/binding参照は下記の式へ展開し、provenanceとarenaそのものは含めない。styleにはschema/nameに加えてfallback roleも含める。

readとbindingの式はvariant名を先頭とするarrayで、variantのpayloadはnative公開型のfield順とする。ListOfは`["ListOf",cons,nil,element]`、Builtinは`["Builtin",reader,kind,tokenKind]`とする。kindは`[SchemaRef,localKind]`、operationは`[SchemaRef,name]`、styleは`[selector,SchemaRef,name,fallback]`である。modeは`[name,skipReaders,takePairs]`、formは`[category,spelling,kind,fields,binding,styles]`、leafは`[category,kind,tokenKind,payloadType,binding,styles]`とする。field、skip/take、choice/seq、bindingの子、styleの各列は意味順を保存する。

readerは全名前付きruleを名前順にたどり、直接DAGの共有を出現ごとに展開したpostorder arenaへ置換したうえで、ReaderPlanの既存canonical descriptor形式を使う。rule名でのRefは展開しない。名前付きruleは未使用でも公開宣言として残す。匿名の未到達arena entryは意味正規形に含めないが、package検査はその不正参照・cycleも拒否する。ReadSpec/Bindingも使用元から展開し、匿名entryの共有・配置・未到達収納の差を除く。巨大な展開は共通予算でStoppedを返し、recursive stackに依存しない。

この正規形は実行上の任意の等価性を証明するものではない。例えば異なるreader式への代数的書換えを同一視しない。providerの要求署名はpackageへ含めるが、hostの実装artifact identityは下記の解析Profileへ含める。packageの出自を持つeditor結果はこの意味digestだけでcacheしない。

recoveryは `[defaultUnexpected,rules]` とし、ruleはcategory順の `[category,unexpected,synchronization]`、同期列は宣言順の `[ancestorCategory,kind,spellingOrNull]` とする。回復方針も実行挙動であり、順序やstrategyの変更を意味identityへ反映する。

### 具体実行identity

継続のarena indexと出自の参照先を固定するため、packageは意味identityとは別にexecutionDigestを持つ。hashは `NEPL3-PACKAGE-EXECUTION-1` + zero byte + 具体実行canonical JSON とする。意味正規形に加え、元のReaderPlan descriptor、ReadSpec/Bindingの全arenaと直接参照ID、form/leaf等の宣言配置、宣言のOrigin参照、全provenance source identity/URI、Origin/sourceMap tableを含める。sourceのbytesは検査済みsnapshot digestで固定する。意味上等しいarena再配置やgrammar source位置だけの変更でも、古いframeや診断originを再利用しない。

EntryContextはpackage意味identityに加えてProfile内のaliasを保持する。同じpackageを異なるcategory-mode overrideで複数登録できるため、packageから最初のaliasを逆引きしてはならない。NodeSelectionのform/leaf/read/binding indexはそのaliasが指すexecutionDigestの実tableに属する。再開および後段での利用時に具体digestを照合し、意味digestが一致する別配置へ勝手に差し替えない。

## 2. native値とwire値

型名中のU64/Bytes32等の有限primitiveと、任意精度Natural/Integerを区別する。wire sourceはopaque SourceId/revision/digestを使う。同じIDをbundle内・操作間の対応づけに使用し、URIやnative allocation addressへ置き換えない。同じURI/revision/byte列を持つ独立文書もSourceIdが異なれば別snapshotである。r3のURI-based bijectionは、この場合にspec02の宣言同一性を失うためr4で訂正した。

native node indexはallocationごとのIDでもよい。wire bundleではrootからfield順に訪問した最初の出現順で連番にする。shared nodeは二回目以降referenceを使う。source tableはSourceIdのUnicode scalar順、revisionの数値順、digestのbyte順とし、schema tableはpackage/revision/digest順。Originの親参照はDAGを検査し、payload nodeとorigin nodeのID空間を分ける。SourceIdはこのnode index再採番の対象にしない。

NodeRefの訪問はdepth-firstで、Childをその位置、Childrenを列の順にたどる。rootは0となり、全NodeRefを書き換える。ForeignSyntaxはguest bundleで独立して再採番し、ForeignSyntax.rootもguestの0へ対応させる。wire bundleは単一rootの到達閉包を表すため、到達不能nodeはUnreachableNodeで拒否し、黙って破棄しない。native arenaの未使用slotは許せるが、出力対象bundleへ含めない。Missing/Unexpected/Unparsed等の回復構文もrootから参照して保持する。

TokenRef、Token内のViewRef、OriginRef、EnvironmentEntry.idはそれぞれの所有tableで宣言されたslotを指し、このNodeRef再採番の対象ではない。これらのtableとbinding/resource/role/relation/triviaの列順は値の一部として保存する。生成側はsource順または明示した生成順でtableを作り、allocation address・hash map列挙順を宣言順へ使わない。したがって、node arenaだけを並べ替えた同じ値はwire byte一致を要求するが、別tableの宣言順まで異なるgraphの同型性をこの規則だけで証明したとは扱わない。

ParseTreeのcontexts/recoveryも同じ所有bundleのNodeRef対応表で変換する。ForeignStep.nodeはそのstepをたどる直前のbundle、NodeSelection.nodeとRecoveryEntry.nodeはpathの終点bundleに属する。contextsはhostを先頭とし、各bundle内の正準node順・field順で出会うforeign bundleをdepth-firstでたどった順に置く。各contextのselectionsは正準node順とする。recoveryは同じbundle順の部分列で、各entriesは正準node順とする。受信側はこの順序と参照先の意味検査を両方行い、古いwrapper番号だけを残した値を拒否する。packageのform/leaf/read/binding index、executionDigest、FactSetのopaque IDは構文node再採番の対象ではない。

coreのNodeMappingは所有bundleの参照範囲・到達閉包と正準番号の対応を計算する。これは単独ではschema・source・cycle・選択の検査済みproofではない。engineのParseTree型付きadapterは解決済みProfileによる静的選択・回復検査に加え、Dynamicの実provider登録・固定shape・保存childContextsの整合を検査し、core-owned FoundationValueCodecを介してwireの実SyntaxBundle変換を使う。wireからengineへの依存は追加しない。Dynamicのpackage indexやchildContextsを構文node番号として再配置しない。この検査はproviderの実行履歴の認証を代替せず、ParseTree codec成立からparse continuation全体のportable化を推定しない。Facts要求応答の型付き包絡はこの同じtree codecと、要求に束縛したfacts権限検査を使用する。

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

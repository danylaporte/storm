(function() {var type_impls = {
"storm_mssql":[["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-Client%3CS%3E\" class=\"impl\"><a href=\"#impl-Client%3CS%3E\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;S&gt; Client&lt;S&gt;<div class=\"where\">where\n    S: AsyncRead + AsyncWrite + <a class=\"trait\" href=\"https://doc.rust-lang.org/1.76.0/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> + <a class=\"trait\" href=\"https://doc.rust-lang.org/1.76.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a>,</div></h3></section></summary><div class=\"impl-items\"><details class=\"toggle method-toggle\" open><summary><section id=\"method.connect\" class=\"method\"><h4 class=\"code-header\">pub async fn <a class=\"fn\">connect</a>(config: Config, tcp_stream: S) -&gt; <a class=\"enum\" href=\"https://doc.rust-lang.org/1.76.0/core/result/enum.Result.html\" title=\"enum core::result::Result\">Result</a>&lt;Client&lt;S&gt;, Error&gt;</h4></section></summary><div class=\"docblock\"><p>Uses an instance of <a href=\"struct.Config.html\"><code>Config</code></a> to specify the connection\noptions required to connect to the database using an established\ntcp connection</p>\n</div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.execute\" class=\"method\"><h4 class=\"code-header\">pub async fn <a class=\"fn\">execute</a>&lt;'a&gt;(\n    &amp;mut self,\n    query: impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.76.0/core/convert/trait.Into.html\" title=\"trait core::convert::Into\">Into</a>&lt;<a class=\"enum\" href=\"https://doc.rust-lang.org/1.76.0/alloc/borrow/enum.Cow.html\" title=\"enum alloc::borrow::Cow\">Cow</a>&lt;'a, <a class=\"primitive\" href=\"https://doc.rust-lang.org/1.76.0/std/primitive.str.html\">str</a>&gt;&gt;,\n    params: &amp;[&amp;dyn ToSql]\n) -&gt; <a class=\"enum\" href=\"https://doc.rust-lang.org/1.76.0/core/result/enum.Result.html\" title=\"enum core::result::Result\">Result</a>&lt;ExecuteResult, Error&gt;</h4></section></summary><div class=\"docblock\"><p>Executes SQL statements in the SQL Server, returning the number rows\naffected. Useful for <code>INSERT</code>, <code>UPDATE</code> and <code>DELETE</code> statements. The\n<code>query</code> can define the parameter placement by annotating them with\n<code>@PN</code>, where N is the index of the parameter, starting from <code>1</code>. If\nexecuting multiple queries at a time, delimit them with <code>;</code> and refer to\n<a href=\"struct.ExecuteResult.html\"><code>ExecuteResult</code></a> how to get results for the separate queries.</p>\n<p>For mapping of Rust types when writing, see the documentation for\n<a href=\"trait.ToSql.html\"><code>ToSql</code></a>. For reading data from the database, see the documentation for\n<a href=\"trait.FromSql.html\"><code>FromSql</code></a>.</p>\n<p>This API is not quite suitable for dynamic query parameters. In these\ncases using a <a href=\"struct.Query.html\"><code>Query</code></a> object might be easier.</p>\n<h5 id=\"example\"><a href=\"#example\">Example</a></h5>\n<div class=\"example-wrap\"><pre class=\"rust rust-example-rendered\"><code><span class=\"kw\">let </span>results = client\n    .execute(\n        <span class=\"string\">\"INSERT INTO ##Test (id) VALUES (@P1), (@P2), (@P3)\"</span>,\n        <span class=\"kw-2\">&amp;</span>[<span class=\"kw-2\">&amp;</span><span class=\"number\">1i32</span>, <span class=\"kw-2\">&amp;</span><span class=\"number\">2i32</span>, <span class=\"kw-2\">&amp;</span><span class=\"number\">3i32</span>],\n    )\n    .<span class=\"kw\">await</span><span class=\"question-mark\">?</span>;</code></pre></div>\n</div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.query\" class=\"method\"><h4 class=\"code-header\">pub async fn <a class=\"fn\">query</a>&lt;'a, 'b&gt;(\n    &amp;'a mut self,\n    query: impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.76.0/core/convert/trait.Into.html\" title=\"trait core::convert::Into\">Into</a>&lt;<a class=\"enum\" href=\"https://doc.rust-lang.org/1.76.0/alloc/borrow/enum.Cow.html\" title=\"enum alloc::borrow::Cow\">Cow</a>&lt;'b, <a class=\"primitive\" href=\"https://doc.rust-lang.org/1.76.0/std/primitive.str.html\">str</a>&gt;&gt;,\n    params: &amp;'b [&amp;'b dyn ToSql]\n) -&gt; <a class=\"enum\" href=\"https://doc.rust-lang.org/1.76.0/core/result/enum.Result.html\" title=\"enum core::result::Result\">Result</a>&lt;QueryStream&lt;'a&gt;, Error&gt;<div class=\"where\">where\n    'a: 'b,</div></h4></section></summary><div class=\"docblock\"><p>Executes SQL statements in the SQL Server, returning resulting rows.\nUseful for <code>SELECT</code> statements. The <code>query</code> can define the parameter\nplacement by annotating them with <code>@PN</code>, where N is the index of the\nparameter, starting from <code>1</code>. If executing multiple queries at a time,\ndelimit them with <code>;</code> and refer to <a href=\"struct.QueryStream.html\"><code>QueryStream</code></a> on proper stream\nhandling.</p>\n<p>For mapping of Rust types when writing, see the documentation for\n<a href=\"trait.ToSql.html\"><code>ToSql</code></a>. For reading data from the database, see the documentation for\n<a href=\"trait.FromSql.html\"><code>FromSql</code></a>.</p>\n<p>This API can be cumbersome for dynamic query parameters. In these cases,\nif fighting too much with the compiler, using a <a href=\"struct.Query.html\"><code>Query</code></a> object might be\neasier.</p>\n<h5 id=\"example-1\"><a href=\"#example-1\">Example</a></h5>\n<div class=\"example-wrap\"><pre class=\"rust rust-example-rendered\"><code><span class=\"kw\">let </span>stream = client\n    .query(\n        <span class=\"string\">\"SELECT @P1, @P2, @P3\"</span>,\n        <span class=\"kw-2\">&amp;</span>[<span class=\"kw-2\">&amp;</span><span class=\"number\">1i32</span>, <span class=\"kw-2\">&amp;</span><span class=\"number\">2i32</span>, <span class=\"kw-2\">&amp;</span><span class=\"number\">3i32</span>],\n    )\n    .<span class=\"kw\">await</span><span class=\"question-mark\">?</span>;</code></pre></div>\n</div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.simple_query\" class=\"method\"><h4 class=\"code-header\">pub async fn <a class=\"fn\">simple_query</a>&lt;'a, 'b&gt;(\n    &amp;'a mut self,\n    query: impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.76.0/core/convert/trait.Into.html\" title=\"trait core::convert::Into\">Into</a>&lt;<a class=\"enum\" href=\"https://doc.rust-lang.org/1.76.0/alloc/borrow/enum.Cow.html\" title=\"enum alloc::borrow::Cow\">Cow</a>&lt;'b, <a class=\"primitive\" href=\"https://doc.rust-lang.org/1.76.0/std/primitive.str.html\">str</a>&gt;&gt;\n) -&gt; <a class=\"enum\" href=\"https://doc.rust-lang.org/1.76.0/core/result/enum.Result.html\" title=\"enum core::result::Result\">Result</a>&lt;QueryStream&lt;'a&gt;, Error&gt;<div class=\"where\">where\n    'a: 'b,</div></h4></section></summary><div class=\"docblock\"><p>Execute multiple queries, delimited with <code>;</code> and return multiple result\nsets; one for each query.</p>\n<h5 id=\"example-2\"><a href=\"#example-2\">Example</a></h5>\n<div class=\"example-wrap\"><pre class=\"rust rust-example-rendered\"><code><span class=\"kw\">let </span>row = client.simple_query(<span class=\"string\">\"SELECT 1 AS col\"</span>).<span class=\"kw\">await</span><span class=\"question-mark\">?</span>.into_row().<span class=\"kw\">await</span><span class=\"question-mark\">?</span>.unwrap();\n<span class=\"macro\">assert_eq!</span>(<span class=\"prelude-val\">Some</span>(<span class=\"number\">1i32</span>), row.get(<span class=\"string\">\"col\"</span>));</code></pre></div>\n<h5 id=\"warning\"><a href=\"#warning\">Warning</a></h5>\n<p>Do not use this with any user specified input. Please resort to prepared\nstatements using the <a href=\"#method.query\"><code>query</code></a> method.</p>\n</div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.bulk_insert\" class=\"method\"><h4 class=\"code-header\">pub async fn <a class=\"fn\">bulk_insert</a>&lt;'a&gt;(\n    &amp;'a mut self,\n    table: &amp;'a <a class=\"primitive\" href=\"https://doc.rust-lang.org/1.76.0/std/primitive.str.html\">str</a>\n) -&gt; <a class=\"enum\" href=\"https://doc.rust-lang.org/1.76.0/core/result/enum.Result.html\" title=\"enum core::result::Result\">Result</a>&lt;BulkLoadRequest&lt;'a, S&gt;, Error&gt;</h4></section></summary><div class=\"docblock\"><p>Execute a <code>BULK INSERT</code> statement, efficiantly storing a large number of\nrows to a specified table. Note: make sure the input row follows the same\nschema as the table, otherwise calling <code>send()</code> will return an error.</p>\n<h5 id=\"example-3\"><a href=\"#example-3\">Example</a></h5>\n<div class=\"example-wrap\"><pre class=\"rust rust-example-rendered\"><code><span class=\"kw\">let </span>create_table = <span class=\"string\">r#\"\n    CREATE TABLE ##bulk_test (\n        id INT IDENTITY PRIMARY KEY,\n        val INT NOT NULL\n    )\n\"#</span>;\n\nclient.simple_query(create_table).<span class=\"kw\">await</span><span class=\"question-mark\">?</span>;\n\n<span class=\"comment\">// Start the bulk insert with the client.\n</span><span class=\"kw\">let </span><span class=\"kw-2\">mut </span>req = client.bulk_insert(<span class=\"string\">\"##bulk_test\"</span>).<span class=\"kw\">await</span><span class=\"question-mark\">?</span>;\n\n<span class=\"kw\">for </span>i <span class=\"kw\">in </span>[<span class=\"number\">0i32</span>, <span class=\"number\">1i32</span>, <span class=\"number\">2i32</span>] {\n    <span class=\"kw\">let </span>row = (i).into_row();\n\n    <span class=\"comment\">// The request will handle flushing to the wire in an optimal way,\n    // balancing between memory usage and IO performance.\n    </span>req.send(row).<span class=\"kw\">await</span><span class=\"question-mark\">?</span>;\n}\n\n<span class=\"comment\">// The request must be finalized.\n</span><span class=\"kw\">let </span>res = req.finalize().<span class=\"kw\">await</span><span class=\"question-mark\">?</span>;\n<span class=\"macro\">assert_eq!</span>(<span class=\"number\">3</span>, res.total());</code></pre></div>\n</div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.close\" class=\"method\"><h4 class=\"code-header\">pub async fn <a class=\"fn\">close</a>(self) -&gt; <a class=\"enum\" href=\"https://doc.rust-lang.org/1.76.0/core/result/enum.Result.html\" title=\"enum core::result::Result\">Result</a>&lt;<a class=\"primitive\" href=\"https://doc.rust-lang.org/1.76.0/std/primitive.unit.html\">()</a>, Error&gt;</h4></section></summary><div class=\"docblock\"><p>Closes this database connection explicitly.</p>\n</div></details></div></details>",0,"storm_mssql::Client"],["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-Debug-for-Client%3CS%3E\" class=\"impl\"><a href=\"#impl-Debug-for-Client%3CS%3E\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;S&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.76.0/core/fmt/trait.Debug.html\" title=\"trait core::fmt::Debug\">Debug</a> for Client&lt;S&gt;<div class=\"where\">where\n    S: <a class=\"trait\" href=\"https://doc.rust-lang.org/1.76.0/core/fmt/trait.Debug.html\" title=\"trait core::fmt::Debug\">Debug</a> + AsyncRead + AsyncWrite + <a class=\"trait\" href=\"https://doc.rust-lang.org/1.76.0/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> + <a class=\"trait\" href=\"https://doc.rust-lang.org/1.76.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a>,</div></h3></section></summary><div class=\"impl-items\"><details class=\"toggle method-toggle\" open><summary><section id=\"method.fmt\" class=\"method trait-impl\"><a href=\"#method.fmt\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"https://doc.rust-lang.org/1.76.0/core/fmt/trait.Debug.html#tymethod.fmt\" class=\"fn\">fmt</a>(&amp;self, f: &amp;mut <a class=\"struct\" href=\"https://doc.rust-lang.org/1.76.0/core/fmt/struct.Formatter.html\" title=\"struct core::fmt::Formatter\">Formatter</a>&lt;'_&gt;) -&gt; <a class=\"enum\" href=\"https://doc.rust-lang.org/1.76.0/core/result/enum.Result.html\" title=\"enum core::result::Result\">Result</a>&lt;<a class=\"primitive\" href=\"https://doc.rust-lang.org/1.76.0/std/primitive.unit.html\">()</a>, <a class=\"struct\" href=\"https://doc.rust-lang.org/1.76.0/core/fmt/struct.Error.html\" title=\"struct core::fmt::Error\">Error</a>&gt;</h4></section></summary><div class='docblock'>Formats the value using the given formatter. <a href=\"https://doc.rust-lang.org/1.76.0/core/fmt/trait.Debug.html#tymethod.fmt\">Read more</a></div></details></div></details>","Debug","storm_mssql::Client"]]
};if (window.register_type_impls) {window.register_type_impls(type_impls);} else {window.pending_type_impls = type_impls;}})()
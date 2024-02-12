(function() {var type_impls = {
"storm":[["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-Vars%3CCTX%3E\" class=\"impl\"><a href=\"#impl-Vars%3CCTX%3E\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;CTX&gt; Vars&lt;CTX&gt;<div class=\"where\">where\n    CTX: VarCnt,</div></h3></section></summary><div class=\"impl-items\"><section id=\"method.new\" class=\"method\"><h4 class=\"code-header\">pub fn <a class=\"fn\">new</a>() -&gt; Vars&lt;CTX&gt;</h4></section><section id=\"method.clear\" class=\"method\"><h4 class=\"code-header\">pub fn <a class=\"fn\">clear</a>&lt;T&gt;(&amp;mut self, var: &amp;'static Var&lt;T, CTX&gt;)</h4></section><section id=\"method.get\" class=\"method\"><h4 class=\"code-header\">pub fn <a class=\"fn\">get</a>&lt;T&gt;(&amp;self, var: &amp;'static Var&lt;T, CTX&gt;) -&gt; <a class=\"enum\" href=\"https://doc.rust-lang.org/1.76.0/core/option/enum.Option.html\" title=\"enum core::option::Option\">Option</a>&lt;<a class=\"primitive\" href=\"https://doc.rust-lang.org/1.76.0/std/primitive.reference.html\">&amp;T</a>&gt;</h4></section><section id=\"method.get_mut\" class=\"method\"><h4 class=\"code-header\">pub fn <a class=\"fn\">get_mut</a>&lt;T&gt;(&amp;mut self, var: &amp;'static Var&lt;T, CTX&gt;) -&gt; <a class=\"enum\" href=\"https://doc.rust-lang.org/1.76.0/core/option/enum.Option.html\" title=\"enum core::option::Option\">Option</a>&lt;<a class=\"primitive\" href=\"https://doc.rust-lang.org/1.76.0/std/primitive.reference.html\">&amp;mut T</a>&gt;</h4></section><details class=\"toggle method-toggle\" open><summary><section id=\"method.get_or_init\" class=\"method\"><h4 class=\"code-header\">pub fn <a class=\"fn\">get_or_init</a>&lt;F, T&gt;(&amp;self, var: &amp;'static Var&lt;T, CTX&gt;, init: F) -&gt; <a class=\"primitive\" href=\"https://doc.rust-lang.org/1.76.0/std/primitive.reference.html\">&amp;T</a><div class=\"where\">where\n    F: <a class=\"trait\" href=\"https://doc.rust-lang.org/1.76.0/core/ops/function/trait.FnOnce.html\" title=\"trait core::ops::function::FnOnce\">FnOnce</a>() -&gt; T,</div></h4></section></summary><div class=\"docblock\"><p>call <code>get</code> method first; it’s faster. If the value is not found then call <code>get_or_init</code>.</p>\n</div></details><section id=\"method.get_or_init_mut\" class=\"method\"><h4 class=\"code-header\">pub fn <a class=\"fn\">get_or_init_mut</a>&lt;F, T&gt;(\n    &amp;mut self,\n    var: &amp;'static Var&lt;T, CTX&gt;,\n    init: F\n) -&gt; <a class=\"primitive\" href=\"https://doc.rust-lang.org/1.76.0/std/primitive.reference.html\">&amp;mut T</a><div class=\"where\">where\n    F: <a class=\"trait\" href=\"https://doc.rust-lang.org/1.76.0/core/ops/function/trait.FnOnce.html\" title=\"trait core::ops::function::FnOnce\">FnOnce</a>() -&gt; T,</div></h4></section><section id=\"method.replace\" class=\"method\"><h4 class=\"code-header\">pub fn <a class=\"fn\">replace</a>&lt;T&gt;(\n    &amp;mut self,\n    var: &amp;'static Var&lt;T, CTX&gt;,\n    val: <a class=\"enum\" href=\"https://doc.rust-lang.org/1.76.0/core/option/enum.Option.html\" title=\"enum core::option::Option\">Option</a>&lt;T&gt;\n) -&gt; <a class=\"enum\" href=\"https://doc.rust-lang.org/1.76.0/core/option/enum.Option.html\" title=\"enum core::option::Option\">Option</a>&lt;T&gt;</h4></section></div></details>",0,"storm::accessor::LogsVar","storm::accessor::Vars"],["<section id=\"impl-Sync-for-Vars%3CCTX%3E\" class=\"impl\"><a href=\"#impl-Sync-for-Vars%3CCTX%3E\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;CTX&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.76.0/core/marker/trait.Sync.html\" title=\"trait core::marker::Sync\">Sync</a> for Vars&lt;CTX&gt;</h3></section>","Sync","storm::accessor::LogsVar","storm::accessor::Vars"],["<section id=\"impl-Send-for-Vars%3CCTX%3E\" class=\"impl\"><a href=\"#impl-Send-for-Vars%3CCTX%3E\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;CTX&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.76.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for Vars&lt;CTX&gt;</h3></section>","Send","storm::accessor::LogsVar","storm::accessor::Vars"],["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-Default-for-Vars%3CCTX%3E\" class=\"impl\"><a href=\"#impl-Default-for-Vars%3CCTX%3E\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;CTX&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.76.0/core/default/trait.Default.html\" title=\"trait core::default::Default\">Default</a> for Vars&lt;CTX&gt;<div class=\"where\">where\n    CTX: VarCnt,</div></h3></section></summary><div class=\"impl-items\"><details class=\"toggle method-toggle\" open><summary><section id=\"method.default\" class=\"method trait-impl\"><a href=\"#method.default\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"https://doc.rust-lang.org/1.76.0/core/default/trait.Default.html#tymethod.default\" class=\"fn\">default</a>() -&gt; Vars&lt;CTX&gt;</h4></section></summary><div class='docblock'>Returns the “default value” for a type. <a href=\"https://doc.rust-lang.org/1.76.0/core/default/trait.Default.html#tymethod.default\">Read more</a></div></details></div></details>","Default","storm::accessor::LogsVar","storm::accessor::Vars"]]
};if (window.register_type_impls) {window.register_type_impls(type_impls);} else {window.pending_type_impls = type_impls;}})()
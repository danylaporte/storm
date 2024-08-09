(function() {var type_impls = {
"storm":[["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-Container%3CCtx%3E\" class=\"impl\"><a href=\"#impl-Container%3CCtx%3E\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;Ctx&gt; Container&lt;Ctx&gt;<div class=\"where\">where\n    Ctx: VarRegister,</div></h3></section></summary><div class=\"impl-items\"><section id=\"method.new\" class=\"method\"><h4 class=\"code-header\">pub fn <a class=\"fn\">new</a>() -&gt; Container&lt;Ctx&gt;</h4></section><section id=\"method.clear\" class=\"method\"><h4 class=\"code-header\">pub fn <a class=\"fn\">clear</a>&lt;T&gt;(&amp;mut self, var: Var&lt;T, Ctx&gt;)</h4></section><section id=\"method.get\" class=\"method\"><h4 class=\"code-header\">pub fn <a class=\"fn\">get</a>&lt;T&gt;(&amp;self, var: Var&lt;T, Ctx&gt;) -&gt; <a class=\"enum\" href=\"https://doc.rust-lang.org/1.80.1/core/option/enum.Option.html\" title=\"enum core::option::Option\">Option</a>&lt;<a class=\"primitive\" href=\"https://doc.rust-lang.org/1.80.1/std/primitive.reference.html\">&amp;T</a>&gt;</h4></section><section id=\"method.get_mut\" class=\"method\"><h4 class=\"code-header\">pub fn <a class=\"fn\">get_mut</a>&lt;T&gt;(&amp;mut self, var: Var&lt;T, Ctx&gt;) -&gt; <a class=\"enum\" href=\"https://doc.rust-lang.org/1.80.1/core/option/enum.Option.html\" title=\"enum core::option::Option\">Option</a>&lt;<a class=\"primitive\" href=\"https://doc.rust-lang.org/1.80.1/std/primitive.reference.html\">&amp;mut T</a>&gt;</h4></section><section id=\"method.get_or_init\" class=\"method\"><h4 class=\"code-header\">pub fn <a class=\"fn\">get_or_init</a>&lt;F, T&gt;(&amp;self, var: Var&lt;T, Ctx&gt;, init: F) -&gt; <a class=\"primitive\" href=\"https://doc.rust-lang.org/1.80.1/std/primitive.reference.html\">&amp;T</a><div class=\"where\">where\n    F: <a class=\"trait\" href=\"https://doc.rust-lang.org/1.80.1/core/ops/function/trait.FnOnce.html\" title=\"trait core::ops::function::FnOnce\">FnOnce</a>() -&gt; T,</div></h4></section><section id=\"method.get_or_init_mut\" class=\"method\"><h4 class=\"code-header\">pub fn <a class=\"fn\">get_or_init_mut</a>&lt;F, T&gt;(&amp;mut self, var: Var&lt;T, Ctx&gt;, init: F) -&gt; <a class=\"primitive\" href=\"https://doc.rust-lang.org/1.80.1/std/primitive.reference.html\">&amp;mut T</a><div class=\"where\">where\n    F: <a class=\"trait\" href=\"https://doc.rust-lang.org/1.80.1/core/ops/function/trait.FnOnce.html\" title=\"trait core::ops::function::FnOnce\">FnOnce</a>() -&gt; T,</div></h4></section><section id=\"method.replace\" class=\"method\"><h4 class=\"code-header\">pub fn <a class=\"fn\">replace</a>&lt;T&gt;(&amp;mut self, var: Var&lt;T, Ctx&gt;, val: <a class=\"enum\" href=\"https://doc.rust-lang.org/1.80.1/core/option/enum.Option.html\" title=\"enum core::option::Option\">Option</a>&lt;T&gt;) -&gt; <a class=\"enum\" href=\"https://doc.rust-lang.org/1.80.1/core/option/enum.Option.html\" title=\"enum core::option::Option\">Option</a>&lt;T&gt;</h4></section></div></details>",0,"storm::accessor::LogsVar","storm::accessor::Vars"],["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-Default-for-Container%3CCtx%3E\" class=\"impl\"><a href=\"#impl-Default-for-Container%3CCtx%3E\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;Ctx&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.80.1/core/default/trait.Default.html\" title=\"trait core::default::Default\">Default</a> for Container&lt;Ctx&gt;<div class=\"where\">where\n    Ctx: VarRegister,</div></h3></section></summary><div class=\"impl-items\"><details class=\"toggle method-toggle\" open><summary><section id=\"method.default\" class=\"method trait-impl\"><a href=\"#method.default\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"https://doc.rust-lang.org/1.80.1/core/default/trait.Default.html#tymethod.default\" class=\"fn\">default</a>() -&gt; Container&lt;Ctx&gt;</h4></section></summary><div class='docblock'>Returns the “default value” for a type. <a href=\"https://doc.rust-lang.org/1.80.1/core/default/trait.Default.html#tymethod.default\">Read more</a></div></details></div></details>","Default","storm::accessor::LogsVar","storm::accessor::Vars"],["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-Drop-for-Container%3CCtx%3E\" class=\"impl\"><a href=\"#impl-Drop-for-Container%3CCtx%3E\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;Ctx&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.80.1/core/ops/drop/trait.Drop.html\" title=\"trait core::ops::drop::Drop\">Drop</a> for Container&lt;Ctx&gt;<div class=\"where\">where\n    Ctx: VarRegister,</div></h3></section></summary><div class=\"impl-items\"><details class=\"toggle method-toggle\" open><summary><section id=\"method.drop\" class=\"method trait-impl\"><a href=\"#method.drop\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"https://doc.rust-lang.org/1.80.1/core/ops/drop/trait.Drop.html#tymethod.drop\" class=\"fn\">drop</a>(&amp;mut self)</h4></section></summary><div class='docblock'>Executes the destructor for this type. <a href=\"https://doc.rust-lang.org/1.80.1/core/ops/drop/trait.Drop.html#tymethod.drop\">Read more</a></div></details></div></details>","Drop","storm::accessor::LogsVar","storm::accessor::Vars"],["<section id=\"impl-Send-for-Container%3CCtx%3E\" class=\"impl\"><a href=\"#impl-Send-for-Container%3CCtx%3E\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;Ctx&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.80.1/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for Container&lt;Ctx&gt;<div class=\"where\">where\n    Ctx: VarRegister,</div></h3></section>","Send","storm::accessor::LogsVar","storm::accessor::Vars"],["<section id=\"impl-Sync-for-Container%3CCtx%3E\" class=\"impl\"><a href=\"#impl-Sync-for-Container%3CCtx%3E\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;Ctx&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.80.1/core/marker/trait.Sync.html\" title=\"trait core::marker::Sync\">Sync</a> for Container&lt;Ctx&gt;<div class=\"where\">where\n    Ctx: VarRegister,</div></h3></section>","Sync","storm::accessor::LogsVar","storm::accessor::Vars"]]
};if (window.register_type_impls) {window.register_type_impls(type_impls);} else {window.pending_type_impls = type_impls;}})()
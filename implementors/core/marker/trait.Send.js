(function() {var implementors = {
"storm":[["impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.66.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"struct\" href=\"storm/prelude/struct.Ctx.html\" title=\"struct storm::prelude::Ctx\">Ctx</a>",1,["storm::ctx::Ctx"]],["impl&lt;'a, L&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.66.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"struct\" href=\"storm/struct.CtxLocks.html\" title=\"struct storm::CtxLocks\">CtxLocks</a>&lt;'a, L&gt;<span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;L: <a class=\"trait\" href=\"https://doc.rust-lang.org/1.66.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a>,</span>",1,["storm::ctx::CtxLocks"]],["impl&lt;'a&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.66.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"struct\" href=\"storm/struct.CtxTransaction.html\" title=\"struct storm::CtxTransaction\">CtxTransaction</a>&lt;'a&gt;",1,["storm::ctx::CtxTransaction"]],["impl&lt;'a, 'b, E&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.66.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"struct\" href=\"storm/struct.TblTransaction.html\" title=\"struct storm::TblTransaction\">TblTransaction</a>&lt;'a, 'b, E&gt;",1,["storm::ctx::TblTransaction"]],["impl&lt;Fields&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.66.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"enum\" href=\"storm/enum.FieldsOrStr.html\" title=\"enum storm::FieldsOrStr\">FieldsOrStr</a>&lt;Fields&gt;<span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;Fields: <a class=\"trait\" href=\"https://doc.rust-lang.org/1.66.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a>,</span>",1,["storm::entity_fields::FieldsOrStr"]],["impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.66.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"enum\" href=\"storm/enum.Error.html\" title=\"enum storm::Error\">Error</a>",1,["storm::error::Error"]],["impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.66.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"struct\" href=\"storm/gc/struct.GcCtx.html\" title=\"struct storm::gc::GcCtx\">GcCtx</a>",1,["storm::gc::GcCtx"]],["impl&lt;E&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.66.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"struct\" href=\"storm/prelude/struct.HashTable.html\" title=\"struct storm::prelude::HashTable\">HashTable</a>&lt;E&gt;",1,["storm::hash_table::HashTable"]],["impl&lt;F&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.66.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"struct\" href=\"storm/struct.InstrumentedErr.html\" title=\"struct storm::InstrumentedErr\">InstrumentedErr</a>&lt;F&gt;<span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;F: <a class=\"trait\" href=\"https://doc.rust-lang.org/1.66.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a>,</span>",1,["storm::instrumented_err::InstrumentedErr"]],["impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.66.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"struct\" href=\"storm/struct.Logs.html\" title=\"struct storm::Logs\">Logs</a>",1,["storm::logs::Logs"]],["impl&lt;E&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.66.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"struct\" href=\"storm/struct.OnRemove.html\" title=\"struct storm::OnRemove\">OnRemove</a>&lt;E&gt;",1,["storm::on_remove::OnRemove"]],["impl&lt;ONE, MANY&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.66.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"struct\" href=\"storm/struct.OneToMany.html\" title=\"struct storm::OneToMany\">OneToMany</a>&lt;ONE, MANY&gt;<span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;MANY: <a class=\"trait\" href=\"https://doc.rust-lang.org/1.66.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a>,<br>&nbsp;&nbsp;&nbsp;&nbsp;ONE: <a class=\"trait\" href=\"https://doc.rust-lang.org/1.66.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a>,</span>",1,["storm::one_to_many::OneToMany"]],["impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.66.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"struct\" href=\"storm/prelude/struct.ProviderContainer.html\" title=\"struct storm::prelude::ProviderContainer\">ProviderContainer</a>",1,["storm::provider::provider_container::ProviderContainer"]],["impl&lt;E&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.66.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"struct\" href=\"storm/prelude/struct.VecTable.html\" title=\"struct storm::prelude::VecTable\">VecTable</a>&lt;E&gt;",1,["storm::vec_table::VecTable"]],["impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.66.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"struct\" href=\"storm/provider/struct.LoadArgs.html\" title=\"struct storm::provider::LoadArgs\">LoadArgs</a>",1,["storm::provider::load_all::LoadArgs"]],["impl&lt;E&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.66.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"struct\" href=\"storm/provider/struct.LoadAllKeyOnly.html\" title=\"struct storm::provider::LoadAllKeyOnly\">LoadAllKeyOnly</a>&lt;E&gt;",1,["storm::provider::load_all::LoadAllKeyOnly"]],["impl&lt;'a&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.66.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"struct\" href=\"storm/provider/struct.TransactionProvider.html\" title=\"struct storm::provider::TransactionProvider\">TransactionProvider</a>&lt;'a&gt;",1,["storm::provider::transaction_provider::TransactionProvider"]],["impl&lt;T&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.66.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"enum\" href=\"storm/enum.LogState.html\" title=\"enum storm::LogState\">LogState</a>&lt;T&gt;<span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;T: <a class=\"trait\" href=\"https://doc.rust-lang.org/1.66.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a>,</span>",1,["storm::state::LogState"]]],
"storm_mssql":[["impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.66.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"struct\" href=\"storm_mssql/struct.ExecuteArgs.html\" title=\"struct storm_mssql::ExecuteArgs\">ExecuteArgs</a>",1,["storm_mssql::execute::ExecuteArgs"]],["impl&lt;'a, K&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.66.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"struct\" href=\"storm_mssql/struct.KeysFilter.html\" title=\"struct storm_mssql::KeysFilter\">KeysFilter</a>&lt;'a, K&gt;<span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;K: <a class=\"trait\" href=\"https://doc.rust-lang.org/1.66.0/core/marker/trait.Sync.html\" title=\"trait core::marker::Sync\">Sync</a>,</span>",1,["storm_mssql::filter_sql::KeysFilter"]],["impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.66.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"struct\" href=\"storm_mssql/struct.MssqlFactory.html\" title=\"struct storm_mssql::MssqlFactory\">MssqlFactory</a>",1,["storm_mssql::mssql_factory::MssqlFactory"]],["impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.66.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"struct\" href=\"storm_mssql/struct.MssqlProvider.html\" title=\"struct storm_mssql::MssqlProvider\">MssqlProvider</a>",1,["storm_mssql::mssql_provider::MssqlProvider"]],["impl&lt;'a&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.66.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"struct\" href=\"storm_mssql/struct.Parameter.html\" title=\"struct storm_mssql::Parameter\">Parameter</a>&lt;'a&gt;",1,["storm_mssql::parameter::Parameter"]],["impl&lt;F&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.66.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"struct\" href=\"storm_mssql/struct.TransactionScoped.html\" title=\"struct storm_mssql::TransactionScoped\">TransactionScoped</a>&lt;F&gt;<span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;F: <a class=\"trait\" href=\"https://doc.rust-lang.org/1.66.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a>,</span>",1,["storm_mssql::transaction_scoped::TransactionScoped"]],["impl&lt;'a&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.66.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"struct\" href=\"storm_mssql/struct.UpsertBuilder.html\" title=\"struct storm_mssql::UpsertBuilder\">UpsertBuilder</a>&lt;'a&gt;",1,["storm_mssql::upsert_builder::UpsertBuilder"]]]
};if (window.register_implementors) {window.register_implementors(implementors);} else {window.pending_implementors = implementors;}})()
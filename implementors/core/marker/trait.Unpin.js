(function() {var implementors = {};
implementors["storm"] = [{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"storm/vars/struct.Tbl.html\" title=\"struct storm::vars::Tbl\">Tbl</a>","synthetic":true,"types":["storm::accessor::vars::Tbl"]},{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"storm/vars/struct.Log.html\" title=\"struct storm::vars::Log\">Log</a>","synthetic":true,"types":["storm::accessor::vars::Log"]},{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"storm/prelude/struct.Ctx.html\" title=\"struct storm::prelude::Ctx\">Ctx</a>","synthetic":true,"types":["storm::ctx::Ctx"]},{"text":"impl&lt;'a, L&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"storm/struct.CtxLocks.html\" title=\"struct storm::CtxLocks\">CtxLocks</a>&lt;'a, L&gt; <span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;L: <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a>,&nbsp;</span>","synthetic":true,"types":["storm::ctx::CtxLocks"]},{"text":"impl&lt;'a&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"storm/struct.CtxTransaction.html\" title=\"struct storm::CtxTransaction\">CtxTransaction</a>&lt;'a&gt;","synthetic":true,"types":["storm::ctx::CtxTransaction"]},{"text":"impl&lt;'a, E&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"storm/struct.TblTransaction.html\" title=\"struct storm::TblTransaction\">TblTransaction</a>&lt;'a, E&gt;","synthetic":true,"types":["storm::ctx::TblTransaction"]},{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"enum\" href=\"storm/enum.Error.html\" title=\"enum storm::Error\">Error</a>","synthetic":true,"types":["storm::error::Error"]},{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"storm/gc/struct.GcCtx.html\" title=\"struct storm::gc::GcCtx\">GcCtx</a>","synthetic":true,"types":["storm::gc::GcCtx"]},{"text":"impl&lt;E&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"storm/prelude/struct.HashTable.html\" title=\"struct storm::prelude::HashTable\">HashTable</a>&lt;E&gt; <span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;E: <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a>,<br>&nbsp;&nbsp;&nbsp;&nbsp;&lt;E as <a class=\"trait\" href=\"storm/prelude/trait.Entity.html\" title=\"trait storm::prelude::Entity\">Entity</a>&gt;::<a class=\"type\" href=\"storm/prelude/trait.Entity.html#associatedtype.Key\" title=\"type storm::prelude::Entity::Key\">Key</a>: <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a>,&nbsp;</span>","synthetic":true,"types":["storm::hash_table::HashTable"]},{"text":"impl&lt;ONE, MANY&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"storm/struct.OneToMany.html\" title=\"struct storm::OneToMany\">OneToMany</a>&lt;ONE, MANY&gt; <span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;ONE: <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a>,&nbsp;</span>","synthetic":true,"types":["storm::one_to_many::OneToMany"]},{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"storm/prelude/struct.ProviderContainer.html\" title=\"struct storm::prelude::ProviderContainer\">ProviderContainer</a>","synthetic":true,"types":["storm::provider::provider_container::ProviderContainer"]},{"text":"impl&lt;E&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"storm/prelude/struct.VecTable.html\" title=\"struct storm::prelude::VecTable\">VecTable</a>&lt;E&gt; <span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;E: <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a>,<br>&nbsp;&nbsp;&nbsp;&nbsp;&lt;E as <a class=\"trait\" href=\"storm/prelude/trait.Entity.html\" title=\"trait storm::prelude::Entity\">Entity</a>&gt;::<a class=\"type\" href=\"storm/prelude/trait.Entity.html#associatedtype.Key\" title=\"type storm::prelude::Entity::Key\">Key</a>: <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a>,&nbsp;</span>","synthetic":true,"types":["storm::vec_table::VecTable"]},{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"storm/provider/struct.LoadArgs.html\" title=\"struct storm::provider::LoadArgs\">LoadArgs</a>","synthetic":true,"types":["storm::provider::load_all::LoadArgs"]},{"text":"impl&lt;E&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"storm/provider/struct.LoadAllKeyOnly.html\" title=\"struct storm::provider::LoadAllKeyOnly\">LoadAllKeyOnly</a>&lt;E&gt; <span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;&lt;E as <a class=\"trait\" href=\"storm/prelude/trait.Entity.html\" title=\"trait storm::prelude::Entity\">Entity</a>&gt;::<a class=\"type\" href=\"storm/prelude/trait.Entity.html#associatedtype.Key\" title=\"type storm::prelude::Entity::Key\">Key</a>: <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a>,&nbsp;</span>","synthetic":true,"types":["storm::provider::load_all::LoadAllKeyOnly"]},{"text":"impl&lt;'a&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"storm/provider/struct.TransactionProvider.html\" title=\"struct storm::provider::TransactionProvider\">TransactionProvider</a>&lt;'a&gt;","synthetic":true,"types":["storm::provider::transaction_provider::TransactionProvider"]},{"text":"impl&lt;T&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"enum\" href=\"storm/enum.LogState.html\" title=\"enum storm::LogState\">LogState</a>&lt;T&gt; <span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;T: <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a>,&nbsp;</span>","synthetic":true,"types":["storm::state::LogState"]}];
implementors["storm_mssql"] = [{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"storm_mssql/struct.ExecuteArgs.html\" title=\"struct storm_mssql::ExecuteArgs\">ExecuteArgs</a>","synthetic":true,"types":["storm_mssql::execute::ExecuteArgs"]},{"text":"impl&lt;'a, K&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"storm_mssql/struct.KeysFilter.html\" title=\"struct storm_mssql::KeysFilter\">KeysFilter</a>&lt;'a, K&gt;","synthetic":true,"types":["storm_mssql::filter_sql::KeysFilter"]},{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"storm_mssql/struct.MssqlFactory.html\" title=\"struct storm_mssql::MssqlFactory\">MssqlFactory</a>","synthetic":true,"types":["storm_mssql::mssql_factory::MssqlFactory"]},{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"storm_mssql/struct.MssqlProvider.html\" title=\"struct storm_mssql::MssqlProvider\">MssqlProvider</a>","synthetic":true,"types":["storm_mssql::mssql_provider::MssqlProvider"]},{"text":"impl&lt;'a&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"storm_mssql/struct.Parameter.html\" title=\"struct storm_mssql::Parameter\">Parameter</a>&lt;'a&gt;","synthetic":true,"types":["storm_mssql::parameter::Parameter"]},{"text":"impl&lt;'a&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"storm_mssql/struct.UpsertBuilder.html\" title=\"struct storm_mssql::UpsertBuilder\">UpsertBuilder</a>&lt;'a&gt;","synthetic":true,"types":["storm_mssql::upsert_builder::UpsertBuilder"]}];
if (window.register_implementors) {window.register_implementors(implementors);} else {window.pending_implementors = implementors;}})()
#[macro_export]
macro_rules! vec_one_many {
    (@filter_iter $e:expr,) => { $e };
    (@filter_iter $e:expr, |$a:pat_param| $filter:expr) => { $e.filter(|$a| $filter) };

    (@filter_opt $e:expr, $id:ident) => { $e };
    (@filter_opt $e:expr, $id:ident |$a:pat_param| $filter:expr) => { $e.filter(|a| { let $a = ($id, a); $filter }) };

    (@filter_new, $e:expr, $id:ident, $new:ident) => { $e };
    (@filter_new, $e:expr, $id:ident, $new:ident |$a:pat_param| $filter:expr) => {
        if { let $a = ($id, $new); $filter }
        {
            $e;
        }
    };

    (@map_one $id:ident, $new:ident |$v:pat_param| $map:expr) => {{
        let $v = ($id, $new);
        $map
    }};

    (@map_many $id:ident, $new:ident) => {
        *$id
    };
    (@map_many $id:ident, $new:ident |$v:pat_param| $map:expr) => {
        let $v = ($id, $new);
        $map
    };

    ($name:ident, $entity:ty, $from:ty
        $(, filter: |$filter_a:pat_param| $filter_e:expr)?
        $(, map_one: |$map_one_a:pat_param| $map_one_e:expr)?
        $(, map_many: |$map_many_a:pat_param| $map_many_e:expr)?
    ) =>
    {
        paste::paste! {
            #[storm::index]
            async fn [<$name:snake>](ctx: &Ctx) -> Result<storm::VecOneMany<$from, <$entity as storm::Entity>::Key>> {
                <$entity as storm::EntityTrx>::changed().register(&[<$name:snake _on_ $entity:snake _changed>]);
                <$entity as storm::EntityTrx>::cleared().register_clear_obj::<Self>();
                <$entity as storm::EntityTrx>::removed().register(&[<$name:snake _on_ $entity:snake _removed>]);

                let tbl = ctx.tbl_of::<$entity>().await?;

                Ok(vec_one_many!(
                        @filter_iter tbl.iter(),
                        $(|$filter_a| $filter_e)?
                    ).map(|(id, e)| {
                        let key = vec_one_many!(@map_one id, e $(|$map_one_a| $map_one_e)?);
                        let val = vec_one_many!(@map_many id, e $(|$map_many_a| $map_many_e)?);

                        (key, val)
                    })
                    .collect()
                )
            }

            fn [<$name:snake _on_ $entity:snake _changed>]<'a>(trx: &'a mut Trx, id: &'a <$entity as storm::Entity>::Key, new: &'a $entity, track: &'a <$entity as storm::Entity>::TrackCtx) -> storm::BoxFuture<'a, Result<()>> {
                Box::pin(async move {
                    let key = vec_one_many!(@map_one id, new $(|$map_one_a| $map_one_e)?);
                    let new_val = vec_one_many!(@map_many id, new $(|$map_many_a| $map_many_e)?);

                    match vec_one_many!(@filter_opt trx.get_entity::<$entity, _>(id).await?, id $(|$filter_a| $filter_e)?).map(|old| vec_one_many!(@map_many id, old $(|$map_many_a| $map_many_e)?)) {
                        Some(old) => {
                            if old != new_val {
                                if let Some(mut idx) = trx.obj_opt::<$name>() {
                                    idx.remove_key_value(old, id);
                                    vec_one_many!(@filter_new, idx.insert(new_val, *id), id, new $(|$filter_a| $filter_e)?);
                                }
                            }
                        }
                        None => {
                            vec_one_many!(@filter_new, {
                                if let Some(mut idx) = trx.obj_opt::<$name>() {
                                    idx.insert(new_val, *id);
                                }
                            }, id, new $(|$filter_a| $filter_e)?);
                        }
                    }

                    Ok(())
                })
            }

            fn [<$name:snake _on_ $entity:snake _removed>]<'a>(trx: &'a mut Trx, id: &'a <$entity as storm::Entity>::Key, track: &'a <$entity as storm::Entity>::TrackCtx) -> storm::BoxFuture<'a, Result<()>> {
                Box::pin(async move {
                    if let Some(old) = vec_one_many!(@filter_opt trx.get_entity::<$entity, _>(id).await?, id $(|$filter_a| $filter_e)?).map(|old| vec_one_many!(@map_many id, old $(|$map_many_a:ident| $map_many_e:expr)?)) {
                        if let Some(mut idx) = trx.obj_opt::<$name>() {
                            idx.remove_key_value(old, id);
                        }
                    }

                    Ok(())
                })
            }
        }
    };
}

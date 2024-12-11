#[macro_export]
macro_rules! one_many {
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

    ($name:ident: VecOneMany<$one:ty, $many:ty> on $entity:ty
        $(, filter: |$filter_a:pat_param| $filter_e:expr)?
        $(, map_one: |$map_one_a:pat_param| $map_one_e:expr)?
        $(, map_many: |$map_many_a:pat_param| $map_many_e:expr)?
    ) =>
    {
        paste::paste! {
            #[$crate::index]
            async fn [<$name:snake>](ctx: &Ctx) -> $crate::Result<$crate::VecOneMany<$one, $many>> {
                <$entity as $crate::EntityTrx>::changed().register(&[<$name:snake _on_ $entity:snake _changed>]);
                <$entity as $crate::EntityTrx>::cleared().register_clear_obj::<Self>();
                <$entity as $crate::EntityTrx>::removed().register(&[<$name:snake _on_ $entity:snake _removed>]);

                let tbl = ctx.tbl_of::<$entity>().await?;

                Ok($crate::one_many!(
                        @filter_iter tbl.iter(),
                        $(|$filter_a| $filter_e)?
                    ).map(|(id, e)| {
                        let key = $crate::one_many!(@map_one id, e $(|$map_one_a| $map_one_e)?);
                        let val = $crate::one_many!(@map_many id, e $(|$map_many_a| $map_many_e)?);

                        (key, val)
                    })
                    .collect()
                )
            }

            fn [<$name:snake _on_ $entity:snake _changed>]<'a>(trx: &'a mut $crate::Trx, id: &'a <$entity as $crate::Entity>::Key, new: &'a $entity, track: &'a <$entity as $crate::Entity>::TrackCtx) -> $crate::BoxFuture<'a, $crate::Result<()>> {
                Box::pin(async move {
                    let key = $crate::one_many!(@map_one id, new $(|$map_one_a| $map_one_e)?);
                    let new_val = $crate::one_many!(@map_many id, new $(|$map_many_a| $map_many_e)?);

                    match $crate::one_many!(@filter_opt trx.get_entity::<$entity, _>(id).await?, id $(|$filter_a| $filter_e)?).map(|old| $crate::one_many!(@map_many id, old $(|$map_many_a| $map_many_e)?)) {
                        Some(old) => {
                            if old != new_val {
                                if let Some(mut idx) = trx.obj_opt::<$name>() {
                                    idx.remove_key_value(old, id);
                                    $crate::one_many!(@filter_new, idx.insert(new_val, *id), id, new $(|$filter_a| $filter_e)?);
                                }
                            }
                        }
                        None => {
                            $crate::one_many!(@filter_new, {
                                if let Some(mut idx) = trx.obj_opt::<$name>() {
                                    idx.insert(new_val, *id);
                                }
                            }, id, new $(|$filter_a| $filter_e)?);
                        }
                    }

                    Ok(())
                })
            }

            fn [<$name:snake _on_ $entity:snake _removed>]<'a>(trx: &'a mut $crate::Trx, id: &'a <$entity as $crate::Entity>::Key, track: &'a <$entity as $crate::Entity>::TrackCtx) -> $crate::BoxFuture<'a, $crate::Result<()>> {
                Box::pin(async move {
                    if let Some(old) = $crate::one_many!(@filter_opt trx.get_entity::<$entity, _>(id).await?, id $(|$filter_a| $filter_e)?).map(|old| $crate::one_many!(@map_many id, old $(|$map_many_a:ident| $map_many_e:expr)?)) {
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

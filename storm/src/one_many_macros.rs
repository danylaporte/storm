#[macro_export]
macro_rules! one_many {
    (@filter $e:expr,) => { true };
    (@filter $e:expr, |$a:pat_param| $filter:expr) => {{ let $a = $e; $filter }};

    (@filter_map $e:expr, |$map_a:pat_param| $map_e:expr) => { Option::from($crate::one_many!(@map $e, |$map_a| $map_e)) };
    (@filter_map $e:expr, |$filter_a:pat_param| $filter_e:expr, |$map_a:pat_param| $map_e:expr) => {
        if $crate::one_many!(@filter $e, |$filter_a| $filter_e) {
            Option::from($crate::one_many!(@map $e, |$map_a| $map_e))
        } else {
            None
        }
    };

    (@map $e:expr, |$map_a:pat_param| $map_e:expr) => {{ let $map_a = $e; $map_e }};

    ($name:ident: VecOneMany<$one:ty, $many:ty> on $entity:ty, $(filter: |$filter_a:pat_param| $filter_e:expr,)? map: |$map_a:pat_param| $map_e:expr) => {
        paste::paste! {
            #[$crate::index]
            pub async fn [<$name:snake>](ctx: &$crate::Ctx) -> $crate::Result<$crate::VecOneMany<$one, $many>> {
                <$entity as $crate::EntityTrx>::changed().register(&[<$name:snake _on_ $entity:snake _changed>]);
                <$entity as $crate::EntityTrx>::cleared().register_clear_obj::<Self>();
                <$entity as $crate::EntityTrx>::removed().register(&[<$name:snake _on_ $entity:snake _removed>]);

                let tbl = ctx.tbl_of::<$entity>().await?;
                let mut map = $crate::vec_map::VecMap::<$one, Vec<$many>>::new();

                for new in tbl {
                    let new: Option<($one, $many)> = $crate::one_many!(@filter_map new, $(|$filter_a| $filter_e,)? |$map_a| $map_e);

                    if let Some((one, many)) = new {
                        map.entry(one).or_default().push(many);
                    }
                }

                Ok(map.into())
            }

            fn [<$name:snake _on_ $entity:snake _changed>]<'a>(trx: &'a mut $crate::Trx, id: &'a <$entity as $crate::Entity>::Key, new: &'a $entity, track: &'a <$entity as $crate::Entity>::TrackCtx) -> $crate::BoxFuture<'a, $crate::Result<()>> {
                Box::pin(async move {
                    let new: Option<($one, $many)> = $crate::one_many!(@filter_map (id, new), $(|$filter_a| $filter_e,)? |$map_a| $map_e);

                    if let Some(old) = trx.get_entity::<$entity, _>(id).await? {
                        let old: Option<($one, $many)> = $crate::one_many!(@filter_map (id, old), $(|$filter_a| $filter_e,)? |$map_a| $map_e);

                        if let Some(old) = old {
                            if new.map_or(true, |new| new != old) {
                                if let Some(mut idx) = trx.obj_opt::<$name>() {
                                    idx.remove_key_value(old.0, &old.1);

                                    if let Some((one, many)) = new {
                                        idx.insert(one, many);
                                    }
                                }
                            }

                            return Ok(());
                        }
                    }

                    if let Some((one, many)) = new {
                        if let Some(mut idx) = trx.obj_opt::<$name>() {
                            idx.insert(one, many);
                        }
                    }

                    Ok(())
                })
            }

            fn [<$name:snake _on_ $entity:snake _removed>]<'a>(trx: &'a mut $crate::Trx, id: &'a <$entity as $crate::Entity>::Key, track: &'a <$entity as $crate::Entity>::TrackCtx) -> $crate::BoxFuture<'a, $crate::Result<()>> {
                Box::pin(async move {

                    if let Some(old) = trx.get_entity::<$entity, _>(id).await? {
                        let old: Option<($one, $many)> = $crate::one_many!(@filter_map (id, old), $(|$filter_a| $filter_e,)? |$map_a| $map_e);

                        if let Some((one, many)) = old {
                            if let Some(mut idx) = trx.obj_opt::<$name>() {
                                idx.remove_key_value(one, &many);
                            }
                        }
                    }

                    Ok(())
                })
            }
        }
    };
}

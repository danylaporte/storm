#[macro_export]
macro_rules! one_one {
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

    ($name:ident: HashOneOne<$k:ty, $v:ty> on $entity:ty, $(filter: |$filter_a:pat_param| $filter_e:expr,)? map: |$map_a:pat_param| $map_e:expr, $error:expr) => {
        paste::paste! {
            #[$crate::index]
            pub async fn [<$name:snake>](ctx: &$crate::Ctx) -> $crate::Result<$crate::HashOneOne<$k, $v>> {
                <$entity as $crate::EntityTrx>::change().register(&[<__ $name:snake _on_ $entity:snake _change>]);
                <$entity as $crate::EntityTrx>::changed().register(&[<__ $name:snake _on_ $entity:snake _changed>]);
                <$entity as $crate::EntityTrx>::cleared().register_clear_obj::<Self>();
                <$entity as $crate::EntityTrx>::removed().register(&[<__ $name:snake _on_ $entity:snake _removed>]);

                let tbl = ctx.tbl_of::<$entity>().await?;
                let mut map = $crate::FxHashMap::<$k, $v>::with_capacity_and_hasher(tbl.len(), Default::default());

                for new in tbl {
                    let new: Option<(_, _)> = $crate::one_many!(@filter_map new, $(|$filter_a| $filter_e,)? |$map_a| $map_e);

                    if let Some((one, many)) = new {
                        map.insert(one.into(), many.into());
                    }
                }

                Ok(map.into())
            }

            #[doc(hidden)]
            fn [<__ $name:snake _on_ $entity:snake _change>]<'a>(trx: &'a mut $crate::Trx, id: &'a <$entity as $crate::Entity>::Key, new: &'a mut $entity, track: &'a <$entity as $crate::Entity>::TrackCtx) -> $crate::BoxFuture<'a, $crate::Result<()>> {
                Box::pin(async move {
                    let new: Option<(_, _)> = $crate::one_many!(@filter_map (id, new), $(|$filter_a| $filter_e,)? |$map_a| $map_e);

                    if let Some(new) = new {
                        match trx.get_entity::<$entity, _>(id).await? {
                            Some(old) => {
                                let old: Option<(_, _)> = $crate::one_many!(@filter_map (id, old), $(|$filter_a| $filter_e,)? |$map_a| $map_e);

                                if old.map_or(true, |new| new != old) {
                                    if trx.obj::<$name>().await?.contains_key(&new) {
                                        return Err($error.into());
                                    }
                                }
                            },
                            None => {
                                if trx.obj::<$name>().await?.contains_key(&new) {
                                    return Err($error.into());
                                }
                            }
                        }
                    }

                    Ok(())
                })
            }

            #[doc(hidden)]
            fn [<__ $name:snake _on_ $entity:snake _changed>]<'a>(trx: &'a mut $crate::Trx, id: &'a <$entity as $crate::Entity>::Key, new: &'a $entity, track: &'a <$entity as $crate::Entity>::TrackCtx) -> $crate::BoxFuture<'a, $crate::Result<()>> {
                Box::pin(async move {
                    let new: Option<(_, _)> = $crate::one_many!(@filter_map (id, new), $(|$filter_a| $filter_e,)? |$map_a| $map_e);

                    if let Some(old) = trx.get_entity::<$entity, _>(id).await? {
                        let old: Option<(_, _)> = $crate::one_many!(@filter_map (id, old), $(|$filter_a| $filter_e,)? |$map_a| $map_e);

                        if let Some(old) = old {
                            if new.map_or(true, |new| new != old) {
                                if let Some(mut idx) = trx.obj_opt::<$name>() {
                                    idx.remove_key_value(old.0.into(), &old.1);

                                    if let Some((one, many)) = new {
                                        idx.insert(one.into(), many.into());
                                    }
                                }
                            }

                            return Ok(());
                        }
                    }

                    if let Some((one, many)) = new {
                        if let Some(mut idx) = trx.obj_opt::<$name>() {
                            idx.insert(one.into(), many.into());
                        }
                    }

                    Ok(())
                })
            }

            #[doc(hidden)]
            fn [<__ $name:snake _on_ $entity:snake _removed>]<'a>(trx: &'a mut $crate::Trx, id: &'a <$entity as $crate::Entity>::Key, track: &'a <$entity as $crate::Entity>::TrackCtx) -> $crate::BoxFuture<'a, $crate::Result<()>> {
                Box::pin(async move {

                    if let Some(old) = trx.get_entity::<$entity, _>(id).await? {
                        let old: Option<(_, _)> = $crate::one_many!(@filter_map (id, old), $(|$filter_a| $filter_e,)? |$map_a| $map_e);

                        if let Some((one, many)) = old {
                            if let Some(mut idx) = trx.obj_opt::<$name>() {
                                idx.remove_key_value(one.into(), &many);
                            }
                        }
                    }

                    Ok(())
                })
            }
        }
    };
}

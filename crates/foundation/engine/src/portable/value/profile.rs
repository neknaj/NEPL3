use super::*;
use crate::profile::{
    CategoryMode, HeadRegistration, LanguageRegistration, ParseProfile, ResourceIdentity,
};

record_value!(LanguageRegistration,engine,"LanguageRegistration",3,[alias:0,package:1,default_category:2]);
record_value!(CategoryMode,engine,"CategoryMode",3,[alias:0,category:1,mode:2]);
record_value!(HeadRegistration,engine,"HeadRegistration",3,[alias:0,category:1,provider:2]);
record_value!(ResourceIdentity,engine,"ResourceIdentity",2,[id:0,digest:1]);
record_value!(ParseProfile,engine,"ParseProfile",9,[id:0,languages:1,schemas:2,category_modes:3,head_providers:4,providers:5,allowlist:6,resources:7,limits:8]);

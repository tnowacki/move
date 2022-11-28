// Copyright (c) The Move Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{
    diag,
    diagnostics::Diagnostic,
    expansion::ast::ModuleIdent,
    naming::ast::{self as N, TParamID, Type, Type_, Var, Var_},
    parser::ast::FunctionName,
    typing::core::{self, TParamSubst},
};
use move_ir_types::location::*;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

type LambdaMap = BTreeMap<Var_, (N::LValueList, Type, Box<N::Exp>, Type)>;

struct Context<'a, 'b> {
    core: &'a mut core::Context<'b>,
    lambdas: LambdaMap,
    tparam_subst: TParamSubst,
}

pub(crate) fn call(
    context: &mut core::Context,
    call_loc: Loc,
    m: ModuleIdent,
    f: FunctionName,
    type_args_opt: Option<Vec<N::Type>>,
    sp!(_, args): Spanned<Vec<N::Exp>>,
) -> Option<N::Exp> {
    let next_color = context.next_variable_color();
    let macro_def = context.module_info(&m).macro_functions.get(&f)?;
    let (macro_type_params, macro_params, mut macro_body) =
        match recolor_macro(call_loc, &m, &f, macro_def, next_color) {
            Ok(res) => res,
            Err(None) => {
                assert!(context.env.has_errors());
                return None;
            }
            Err(Some(diag)) => {
                context.env.add_diag(*diag);
                return None;
            }
        };
    let type_args = match type_args_opt {
        Some(tys) => tys,
        None => macro_type_params
            .iter()
            .map(|_| core::make_tvar(context, call_loc))
            .collect(),
    };
    if macro_type_params.len() != type_args.len() || macro_params.len() != args.len() {
        assert!(context.env.has_errors());
        return None;
    }
    // make tparam subst
    let tparam_subst = macro_type_params.into_iter().zip(type_args).collect();
    // make lambda map and bind non-lambda args to local vars
    let mut lambdas = BTreeMap::new();
    let mut result = VecDeque::new();
    for ((param, param_ty), arg) in macro_params.into_iter().zip(args) {
        let param_ty = core::subst_tparams(&tparam_subst, param_ty);
        if let sp!(loc, Type_::Fun(param_tys, result_ty)) = param_ty {
            let param_tys = Type_::multiple(loc, param_tys);
            bind_lambda(
                context,
                &mut lambdas,
                param.value,
                arg,
                param_tys,
                *result_ty,
            )?;
        } else {
            // todo var determine usage
            let var_ = N::LValue_::Var {
                var: param,
                unused_binding: false,
            };
            let bind_ = sp(param.loc, var_);
            let bind = sp(param.loc, vec![bind_]);
            let arg_loc = arg.loc;
            let annot_arg = sp(arg_loc, N::Exp_::Annotate(Box::new(arg), param_ty));
            result.push_back(sp(arg_loc, N::SequenceItem_::Bind(bind, annot_arg)));
        }
    }
    let mut context = Context {
        core: context,
        lambdas,
        tparam_subst,
    };
    seq(&mut context, &mut macro_body);
    result.extend(macro_body);
    Some(sp(call_loc, N::Exp_::Block(result)))
}

fn recolor_macro(
    call_loc: Loc,
    m: &ModuleIdent,
    f: &FunctionName,
    macro_def: &N::Function,
    color: u16,
) -> Result<(Vec<TParamID>, Vec<(Var, N::Type)>, N::Sequence), Option<Box<Diagnostic>>> {
    let N::Function {
        macro_,
        signature,
        body,
        ..
    } = macro_def;
    if macro_.is_none() {
        return Err(None);
    }
    let N::FunctionSignature {
        type_parameters,
        parameters,
        ..
    } = signature;
    let tparam_ids = type_parameters.iter().map(|t| t.id).collect();
    let parameters = parameters
        .iter()
        .map(|(v, t)| (recolor_var_owned(None, color, *v), t.clone()))
        .collect();
    let body = match &body.value {
        N::FunctionBody_::Defined(body) => {
            let mut body = body.clone();
            recolor_seq(None, color, &mut body);
            body
        }
        N::FunctionBody_::Native => {
            return Err(Some(Box::new(diag!(
                TypeSafety::InvalidNativeUsage,
                (call_loc, format!("Unknown native macro '{}::{}'", m, f))
            ))));
        }
    };
    Ok((tparam_ids, parameters, body))
}

fn bind_lambda(
    context: &mut core::Context,
    lambdas: &mut LambdaMap,
    param: Var_,
    arg: N::Exp,
    param_ty: Type,
    result_ty: Type,
) -> Option<()> {
    match arg.value {
        N::Exp_::Annotate(inner, _) => {
            bind_lambda(context, lambdas, param, *inner, param_ty, result_ty)
        }
        N::Exp_::Lambda(lvs, body) => {
            lambdas.insert(param, (lvs, param_ty, body, result_ty));
            Some(())
        }
        _ => {
            let msg = format!(
                "Unable to bind lambda to parameter '{}'. The lambda must be passed directly",
                param.name
            );
            context
                .env
                .add_diag(diag!(TypeSafety::CannotExpandMacro, (arg.loc, msg)));
            None
        }
    }
}

//**************************************************************************************************
// recolor
//**************************************************************************************************

fn recolor_var_owned(mask: Option<&BTreeSet<Var_>>, color: u16, mut v: Var) -> Var {
    recolor_var(mask, color, &mut v);
    v
}

fn recolor_var(mask: Option<&BTreeSet<Var_>>, color: u16, v: &mut Var) {
    // do not recolor if a mask is present and not in the mask
    if let Some(mask) = mask {
        if !mask.contains(&v.value) {
            return;
        }
    }
    assert!(v.value.color == 0);
    v.value.color = color;
}

fn recolor_seq(mask: Option<&BTreeSet<Var_>>, color: u16, seq: &mut N::Sequence) {
    for sp!(_, item_) in seq {
        match item_ {
            N::SequenceItem_::Seq(e) => recolor_exp(mask, color, e),
            N::SequenceItem_::Declare(lvalues, _) => recolor_lvalues(mask, color, lvalues),
            N::SequenceItem_::Bind(lvalues, e) => {
                recolor_lvalues(mask, color, lvalues);
                recolor_exp(mask, color, e)
            }
        }
    }
}

fn recolor_lvalues(mask: Option<&BTreeSet<Var_>>, color: u16, sp!(_, lvalues): &mut N::LValueList) {
    for lvalue in lvalues {
        recolor_lvalue(mask, color, lvalue)
    }
}

fn recolor_lvalue(mask: Option<&BTreeSet<Var_>>, color: u16, sp!(_, lvalue_): &mut N::LValue) {
    match lvalue_ {
        N::LValue_::Ignore => (),
        N::LValue_::Var { var, .. } => recolor_var(mask, color, var),
        N::LValue_::Unpack(_, _, _, lvalues) => {
            for (_, _, (_, lvalue)) in lvalues {
                recolor_lvalue(mask, color, lvalue)
            }
        }
    }
}

fn recolor_exp(mask: Option<&BTreeSet<Var_>>, color: u16, sp!(_, e_): &mut N::Exp) {
    match e_ {
        N::Exp_::Value(_)
        | N::Exp_::Constant(_, _)
        | N::Exp_::Break
        | N::Exp_::Continue
        | N::Exp_::Unit { .. }
        | N::Exp_::UnresolvedError => (),
        N::Exp_::Spec(_, var_set) => {
            *var_set = std::mem::take(var_set)
                .into_iter()
                .map(|v| recolor_var_owned(mask, color, v))
                .collect()
        }
        N::Exp_::Move(var) | N::Exp_::Copy(var) | N::Exp_::Use(var) => {
            recolor_var(mask, color, var)
        }
        N::Exp_::Return(e)
        | N::Exp_::Abort(e)
        | N::Exp_::Dereference(e)
        | N::Exp_::UnaryExp(_, e)
        | N::Exp_::Cast(e, _)
        | N::Exp_::Loop(e)
        | N::Exp_::Annotate(e, _) => recolor_exp(mask, color, e),
        N::Exp_::Assign(lvalues, e) => {
            recolor_lvalues(mask, color, lvalues);
            recolor_exp(mask, color, e)
        }
        N::Exp_::IfElse(econd, et, ef) => {
            recolor_exp(mask, color, econd);
            recolor_exp(mask, color, et);
            recolor_exp(mask, color, ef);
        }
        N::Exp_::While(econd, ebody) => {
            recolor_exp(mask, color, econd);
            recolor_exp(mask, color, ebody)
        }
        N::Exp_::Block(s) => recolor_seq(mask, color, s),
        N::Exp_::FieldMutate(ed, e) => {
            recolor_exp_dotted(mask, color, ed);
            recolor_exp(mask, color, e)
        }
        N::Exp_::Mutate(el, er) | N::Exp_::BinopExp(el, _, er) => {
            recolor_exp(mask, color, el);
            recolor_exp(mask, color, er)
        }
        N::Exp_::Pack(_, _, _, fields) => {
            for (_, _, (_, e)) in fields {
                recolor_exp(mask, color, e)
            }
        }
        N::Exp_::Builtin(_, sp!(_, es))
        | N::Exp_::Vector(_, _, sp!(_, es))
        | N::Exp_::ModuleCall(_, _, _, _, sp!(_, es))
        | N::Exp_::ExpList(es) => {
            for e in es {
                recolor_exp(mask, color, e)
            }
        }
        N::Exp_::VarCall(v, sp!(_, es)) => {
            recolor_var(mask, color, v);
            for e in es {
                recolor_exp(mask, color, e)
            }
        }

        N::Exp_::Lambda(lvalues, e) => {
            recolor_lvalues(mask, color, lvalues);
            recolor_exp(mask, color, e)
        }
        N::Exp_::DerefBorrow(ed) | N::Exp_::Borrow(_, ed) => recolor_exp_dotted(mask, color, ed),
    }
}

fn recolor_exp_dotted(mask: Option<&BTreeSet<Var_>>, color: u16, sp!(_, ed_): &mut N::ExpDotted) {
    match ed_ {
        N::ExpDotted_::Exp(e) => recolor_exp(mask, color, e),
        N::ExpDotted_::Dot(ed, _) => recolor_exp_dotted(mask, color, ed),
    }
}

//**************************************************************************************************
// recolor
//**************************************************************************************************

fn types(context: &mut Context, tys: &mut [Type]) {
    for ty in tys {
        type_(context, ty)
    }
}

fn type_(context: &mut Context, ty: &mut N::Type) {
    *ty = core::subst_tparams(&context.tparam_subst, ty.clone())
}

fn seq(context: &mut Context, seq: &mut N::Sequence) {
    for sp!(_, item_) in seq {
        match item_ {
            N::SequenceItem_::Seq(e) => exp(context, e),
            N::SequenceItem_::Declare(lvs, _) => lvalues(context, lvs),
            N::SequenceItem_::Bind(lvs, e) => {
                lvalues(context, lvs);
                exp(context, e)
            }
        }
    }
}

fn lvalues(context: &mut Context, sp!(_, lvs_): &mut N::LValueList) {
    for lv in lvs_ {
        lvalue(context, lv)
    }
}

fn lvalue(context: &mut Context, sp!(_, lv_): &mut N::LValue) {
    match lv_ {
        N::LValue_::Ignore | N::LValue_::Var { .. } => (),
        N::LValue_::Unpack(_, _, tys_opt, lvalues) => {
            if let Some(tys) = tys_opt {
                types(context, tys)
            }
            for (_, _, (_, lv)) in lvalues {
                lvalue(context, lv)
            }
        }
    }
}

fn exp(context: &mut Context, sp!(_, e_): &mut N::Exp) {
    match e_ {
        N::Exp_::Value(_)
        | N::Exp_::Constant(_, _)
        | N::Exp_::Break
        | N::Exp_::Continue
        | N::Exp_::Unit { .. }
        | N::Exp_::UnresolvedError
        | N::Exp_::Spec(_, _)
        | N::Exp_::Move(_)
        | N::Exp_::Copy(_)
        | N::Exp_::Use(_) => (),
        N::Exp_::Return(e)
        | N::Exp_::Abort(e)
        | N::Exp_::Dereference(e)
        | N::Exp_::UnaryExp(_, e)
        | N::Exp_::Loop(e) => exp(context, e),
        N::Exp_::Cast(e, ty) | N::Exp_::Annotate(e, ty) => {
            exp(context, e);
            type_(context, ty)
        }
        N::Exp_::Assign(lvs, e) => {
            lvalues(context, lvs);
            exp(context, e)
        }
        N::Exp_::IfElse(econd, et, ef) => {
            exp(context, econd);
            exp(context, et);
            exp(context, ef);
        }
        N::Exp_::While(econd, ebody) => {
            exp(context, econd);
            exp(context, ebody)
        }
        N::Exp_::Block(s) => seq(context, s),
        N::Exp_::FieldMutate(ed, e) => {
            exp_dotted(context, ed);
            exp(context, e)
        }
        N::Exp_::Mutate(el, er) | N::Exp_::BinopExp(el, _, er) => {
            exp(context, el);
            exp(context, er)
        }
        N::Exp_::Pack(_, _, tys_opt, fields) => {
            if let Some(tys) = tys_opt {
                types(context, tys)
            }
            for (_, _, (_, e)) in fields {
                exp(context, e)
            }
        }
        N::Exp_::Builtin(bf, sp!(_, es)) => {
            builtin(context, bf);
            exps(context, es)
        }
        N::Exp_::Vector(_, ty_opt, sp!(_, es)) => {
            if let Some(ty) = ty_opt {
                type_(context, ty)
            }
            exps(context, es)
        }
        N::Exp_::ModuleCall(_, _, _, tys_opt, sp!(_, es)) => {
            if let Some(tys) = tys_opt {
                types(context, tys)
            }
            exps(context, es)
        }
        N::Exp_::ExpList(es) => exps(context, es),
        N::Exp_::Lambda(lvs, e) => {
            lvalues(context, lvs);
            exp(context, e)
        }
        N::Exp_::DerefBorrow(ed) | N::Exp_::Borrow(_, ed) => exp_dotted(context, ed),
        N::Exp_::VarCall(v, sp!(_, es)) if context.lambdas.contains_key(&v.value) => {
            exps(context, es);
            // param_ty and result_ty have already been substituted
            let (mut lambda_params, param_ty, mut lambda_body, result_ty) =
                context.lambdas.get(&v.value).unwrap().clone();
            // recolor in case the lambda is used more than once
            let mask = make_lambda_mask(&lambda_params);
            let next_color = context.core.next_variable_color();
            recolor_lvalues(Some(&mask), next_color, &mut lambda_params);
            recolor_exp(Some(&mask), next_color, &mut lambda_body);
            let param_loc = lambda_params.loc;
            let N::Exp_::VarCall(_, sp!(args_loc, arg_list)) =
                std::mem::replace(e_, /* dummy */ N::Exp_::UnresolvedError) else { panic!() };
            let args = sp(args_loc, N::Exp_::ExpList(arg_list));
            let annot_args = sp(args_loc, N::Exp_::Annotate(Box::new(args), param_ty));
            let body_loc = lambda_body.loc;
            let annot_body = sp(body_loc, N::Exp_::Annotate(lambda_body, result_ty));
            let result = VecDeque::from([
                sp(param_loc, N::SequenceItem_::Bind(lambda_params, annot_args)),
                sp(body_loc, N::SequenceItem_::Seq(annot_body)),
            ]);
            *e_ = N::Exp_::Block(result);
        }
        N::Exp_::VarCall(_, sp!(_, es)) => exps(context, es),
    }
}

fn builtin(context: &mut Context, sp!(_, bf_): &mut N::BuiltinFunction) {
    match bf_ {
        N::BuiltinFunction_::MoveTo(ty_opt)
        | N::BuiltinFunction_::MoveFrom(ty_opt)
        | N::BuiltinFunction_::BorrowGlobal(_, ty_opt)
        | N::BuiltinFunction_::Exists(ty_opt)
        | N::BuiltinFunction_::Freeze(ty_opt) => {
            if let Some(ty) = ty_opt {
                type_(context, ty)
            }
        }
        N::BuiltinFunction_::Assert(_) => (),
    }
}

fn exp_dotted(context: &mut Context, sp!(_, ed_): &mut N::ExpDotted) {
    match ed_ {
        N::ExpDotted_::Exp(e) => exp(context, e),
        N::ExpDotted_::Dot(ed, _) => exp_dotted(context, ed),
    }
}

fn exps(context: &mut Context, es: &mut [N::Exp]) {
    for e in es {
        exp(context, e)
    }
}

fn make_lambda_mask(lvalues: &N::LValueList) -> BTreeSet<Var_> {
    let mut mask = BTreeSet::new();
    make_lambda_mask_lvalues(&mut mask, lvalues);
    mask
}

fn make_lambda_mask_lvalues(mask: &mut BTreeSet<Var_>, sp!(_, lvs_): &N::LValueList) {
    for lv in lvs_ {
        make_lambda_mask_lvalue(mask, lv)
    }
}

fn make_lambda_mask_lvalue(mask: &mut BTreeSet<Var_>, sp!(_, lv_): &N::LValue) {
    match lv_ {
        N::LValue_::Ignore => (),
        N::LValue_::Var { var, .. } => {
            mask.insert(var.value);
        }
        N::LValue_::Unpack(_, _, _, lvalues) => {
            for (_, _, (_, lv)) in lvalues {
                make_lambda_mask_lvalue(mask, lv)
            }
        }
    }
}

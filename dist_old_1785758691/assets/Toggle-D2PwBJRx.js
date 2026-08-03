import{H as n,d as p,g as d,A as b,i as f,c as _,b as t,n as l,k as g,_ as x}from"./index-COLqXbry.js";/**
 * @license lucide-vue-next v1.0.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */const C=n("square",[["rect",{width:"18",height:"18",x:"3",y:"3",rx:"2",key:"afitv7"}]]);/**
 * @license lucide-vue-next v1.0.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */const M=n("volume-2",[["path",{d:"M11 4.702a.705.705 0 0 0-1.203-.498L6.413 7.587A1.4 1.4 0 0 1 5.416 8H3a1 1 0 0 0-1 1v6a1 1 0 0 0 1 1h2.416a1.4 1.4 0 0 1 .997.413l3.383 3.384A.705.705 0 0 0 11 19.298z",key:"uqj9uw"}],["path",{d:"M16 9a5 5 0 0 1 0 6",key:"1q6k2b"}],["path",{d:"M19.364 18.364a9 9 0 0 0 0-12.728",key:"ijwkga"}]]);/**
 * @license lucide-vue-next v1.0.0 - ISC
 *
 * This source code is licensed under the ISC license.
 * See the LICENSE file in the root directory of this source tree.
 */const q=n("x",[["path",{d:"M18 6 6 18",key:"1bl5f8"}],["path",{d:"m6 6 12 12",key:"d8bk6v"}]]),k={class:"flex items-center"},m=["id","checked","disabled"],w=["for"],v={class:"min-w-0 flex-1"},y=p({__name:"Toggle",props:{checked:{type:Boolean,default:!1},disabled:{type:Boolean,default:!1}},emits:["change"],setup(o,{emit:i}){const c=o,h=i,r=d(`toggle-${Math.random().toString(36).substring(2,9)}`),e=d(c.checked);b(()=>c.checked,a=>{e.value=a});const u=a=>{const s=a.target;e.value=s.checked,h("change",s.checked)};return(a,s)=>(f(),_("div",k,[t("input",{type:"checkbox",id:r.value,checked:e.value,disabled:o.disabled,onChange:u,class:"hidden"},null,40,m),t("label",{for:r.value,class:l(["relative text-white text-3.5 select-none inline-flex items-center w-full",o.disabled?"cursor-not-allowed opacity-50":"cursor-pointer"]),style:{"text-shadow":"0 2px 4px rgba(0, 0, 0, 0.3)"}},[t("span",{class:l(["relative inline-block w-12.5 h-6.5 shrink-0 rounded-[13px] transition-all duration-300 ease-in-out mr-2",[e.value?"border-(--accent-color) bg-[rgba(121,217,255,0.3)] shadow-[0_0_10px_rgba(121,217,255,0.3)]":"border-white/30 bg-white/20"]])},[t("span",{class:l(["absolute top-1/2 -translate-y-1/2 w-5 h-5 rounded-full transition-all duration-300 ease-in-out",[e.value?"left-6.5 bg-linear-to-br from-(--accent-color) to-[#64b5f6] shadow-[0_3px_8px_rgba(121,217,255,0.4),0_1px_3px_rgba(0,0,0,0.2)]":"left-1 bg-linear-to-br from-white to-[#f0f0f0] shadow-[0_2px_6px_rgba(0,0,0,0.3),0_1px_2px_rgba(0,0,0,0.1)]"]])},null,2)],2),t("span",v,[g(a.$slots,"default",{},void 0,!0)])],10,w)]))}}),S=x(y,[["__scopeId","data-v-1faf58f3"]]);export{C as S,S as T,M as V,q as X};

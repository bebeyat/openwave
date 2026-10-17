//! The page-side half of the ambient theme: `window.__owTheme.apply(palette)` / `.clear()`.
//!
//! Two stacked backdrop layers (`#ow-bd-a`, `#ow-bd-b`) hold the blurred cover. A new cover is
//! preloaded, put on the upper layer and faded in over 700ms (CSS transition on opacity); the
//! lower layer is switched off afterwards. The accent variables on `<html>` are registered CSS
//! properties, so setting them also crossfades. Under reduced motion or low power no image layer
//! is used at all: the page gets a flat accent tint (`data-flat`) and no transitions.

pub const THEME_JS: &str = r#"
(function(){
var r=document.documentElement,cur=0,seq=0,timer=0,lastP=null;
var PROPS=['--ow-accent','--ow-accent-2','--ow-on-accent','--ow-at-d','--ow-at-l','--ow-sa-d','--ow-sa-l'];
function layer(i){return document.getElementById(i?'ow-bd-b':'ow-bd-a');}
function reduced(){return r.hasAttribute('data-low-power')||(window.matchMedia&&window.matchMedia('(prefers-reduced-motion: reduce)').matches);}
function paint(p){
  var s=r.style;
  lastP=p;
  r.setAttribute('data-cover','');
  // A fixed accent (Özelleştirme) keeps its colours; the cover still drives the backdrop.
  if(r.hasAttribute('data-fixed-accent')){['--ow-accent','--ow-accent-2','--ow-on-accent','--ow-at-d','--ow-at-l'].forEach(function(k){s.removeProperty(k);});}
  else{
  s.setProperty('--ow-accent',p.a);s.setProperty('--ow-accent-2',p.b);s.setProperty('--ow-on-accent',p.on);
  s.setProperty('--ow-at-d',p.atd);s.setProperty('--ow-at-l',p.atl);
  }
  s.setProperty('--ow-sa-d',String(p.sad));s.setProperty('--ow-sa-l',String(p.sal));
}
function layersOff(){var a=layer(0),b=layer(1);if(a)a.removeAttribute('data-on');if(b)b.removeAttribute('data-on');}
window.__owTheme={
  apply:function(p){
    var my=++seq;clearTimeout(timer);
    if(reduced()||!p.url){layersOff();r.setAttribute('data-flat','');paint(p);return;}
    var img=new Image();
    img.onload=function(){
      if(my!==seq)return;
      var o=layer(cur),n=layer(1-cur);if(!o||!n)return;
      n.style.backgroundImage='url('+JSON.stringify(p.url)+')';
      n.style.zIndex='2';o.style.zIndex='1';
      r.removeAttribute('data-flat');paint(p);
      n.setAttribute('data-on','');
      cur=1-cur;
      timer=setTimeout(function(){if(my===seq)o.removeAttribute('data-on');},760);
    };
    img.onerror=function(){if(my!==seq)return;layersOff();r.setAttribute('data-flat','');paint(p);};
    img.src=p.url;
  },
  refresh:function(){if(lastP&&r.hasAttribute('data-cover'))paint(lastP);},
  clear:function(){
    ++seq;clearTimeout(timer);layersOff();
    PROPS.forEach(function(k){r.style.removeProperty(k);});
    r.removeAttribute('data-cover');r.removeAttribute('data-flat');lastP=null;
  }
};
})();
"#;

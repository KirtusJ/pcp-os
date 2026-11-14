#!/usr/bin/env python3
"""Colour audit for the edition themes: WCAG 2.x contrast, CIELAB lightness and hue, and a
dichromacy simulation (Machado, Oliveira and Fernandes 2009 matrices, severity 1.0) to check that the
three edition fields stay distinguishable without hue."""
import math, json, sys
def srgb(h): h=h.lstrip('#'); return [int(h[i:i+2],16)/255 for i in (0,2,4)]
def lin(c): return c/12.92 if c<=0.04045 else ((c+0.055)/1.055)**2.4
def unlin(c): c=max(0,min(1,c)); return 12.92*c if c<=0.0031308 else 1.055*c**(1/2.4)-0.055
def lum(h): r,g,b=[lin(c) for c in srgb(h)]; return 0.2126*r+0.7152*g+0.0722*b
def contrast(a,b): la,lb=sorted((lum(a),lum(b)),reverse=True); return (la+0.05)/(lb+0.05)
def lab(h):
    r,g,b=[lin(c) for c in srgb(h)]
    X=0.4124564*r+0.3575761*g+0.1804375*b; Y=0.2126729*r+0.7151522*g+0.0721750*b; Z=0.0193339*r+0.1191920*g+0.9503041*b
    f=lambda t: t**(1/3) if t>0.008856 else 7.787*t+16/116
    fx,fy,fz=f(X/0.95047),f(Y),f(Z/1.08883)
    return 116*fy-16, 500*(fx-fy), 200*(fy-fz)
def hue(h): L,a,b=lab(h); return math.degrees(math.atan2(b,a))%360, math.hypot(a,b)
def dE(a,b): return math.dist(lab(a),lab(b))
M={'protan':[[0.152286,1.052583,-0.204868],[0.114503,0.786281,0.099216],[-0.003882,-0.048116,1.051998]],
   'deutan':[[0.367322,0.860646,-0.227968],[0.280085,0.672501,0.047413],[-0.011820,0.042940,0.968881]],
   'tritan':[[1.255528,-0.076749,-0.178779],[-0.078411,0.930809,0.147602],[0.004733,0.691367,0.303900]]}
def sim(h,kind):
    v=[lin(c) for c in srgb(h)]; o=[sum(M[kind][i][j]*v[j] for j in range(3)) for i in range(3)]
    return '#'+''.join('%02X'%round(unlin(c)*255) for c in o)
if __name__=='__main__':
    T=json.load(open(sys.argv[1]))
    for name,t in T.items():
        L,_,_=lab(t['field']); hh,ch=hue(t['sig'])
        print('%-10s field %s L*=%5.1f | text/field %.2f | sig/field %.2f | sig hue %5.1f chroma %4.1f | ink/white %.2f | th-text/th %.2f | note/field %.2f'%(
            name,t['field'],L,contrast(t['text'],t['field']),contrast(t['sig'],t['field']),hh,ch,contrast(t['ink'],'#FFFFFF'),contrast('#FFFFFF',t['th']),contrast(t['soft'],t['field'])))
    names=list(T)
    for i in range(3):
        for j in range(i+1,3):
            a,b=T[names[i]]['field'],T[names[j]]['field']
            print('fields %s vs %s: dE76 %.1f; '%(names[i],names[j],dE(a,b))+'; '.join('%s dE %.1f'%(k,dE(sim(a,k),sim(b,k))) for k in M)+'; dL* %.1f'%abs(lab(a)[0]-lab(b)[0]))

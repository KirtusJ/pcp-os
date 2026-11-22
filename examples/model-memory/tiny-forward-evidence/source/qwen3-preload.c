#include <efi.h>
#include <efilib.h>
#include "manifest.h"
#include "kernel.h"
#include "guests.h"
#define PAGE 4096ULL
#define HUGE (2ULL<<20)
#define SHARD (64ULL<<20)
#define TOTAL 80896ULL
#define LOW_MANIFEST (64ULL<<20)
struct mb_module { UINT32 start,end,name,reserved; };
#define FRONT (8*PAGE)
#define ROUNDED ((TOTAL+PAGE-1)&~(PAGE-1))
#define HIGH (1ULL<<32)
#ifndef MODEL_KIND
#define MODEL_KIND 0x80000001U
#endif
#define FRONT_ADDR (72ULL<<20)
#define READER_ADDR (73ULL<<20)
#define BOOT_ADDR (80ULL<<20)
#define BOOT_LEN (64ULL<<10)
struct __attribute__((packed)) mb_range {UINT32 size; UINT64 base,length;UINT32 type;};
struct boot_data {UINT32 info[32];struct mb_module mods[3];char argv[1024]; char names[3][16]; struct mb_range map[512];};
_Static_assert(sizeof(struct boot_data)<BOOT_LEN-16384,"stack space");
static struct boot_data *const boot=(struct boot_data *)(UINTN)BOOT_ADDR;
static UINTN record_count;
extern void transfer(UINT64 info,UINT64 entry,UINT64 stack) __attribute__((sysv_abi,noreturn));
extern char transfer_end[];
static UINT8 map_data[128*1024] __attribute__((aligned(8)));
static UINTN stride,count,map_key;
static UINT64 model,guard,second;
static inline void emit(char c){__asm__ volatile("outb %0,$0xe9"::"a"(c));}
static void msg(const char *s){while(*s)emit(*s++);}
static void hex(UINT64 x){const char *tbl="0123456789abcdef";for(int i=60;i>=0;i-=4)emit(tbl[(x>>i)&15]);}
static void num(const char *s,UINT64 v){msg(s);emit('=');hex(v);emit('\n');}
static void stop(unsigned v){__asm__ volatile("outl %0,$0xf4"::"a"(v));for(;;)__asm__ volatile("cli;hlt");}
static void check(int c,const char *why){if(!c){msg("refused ");msg(why);emit('\n');stop(18);}}
static EFI_MEMORY_DESCRIPTOR *desc(UINTN i){return (EFI_MEMORY_DESCRIPTOR *)(map_data+i*stride);}
static void get_map(EFI_BOOT_SERVICES *bs){
 UINTN size=sizeof(map_data);UINT32 version;
 EFI_STATUS s=uefi_call_wrapper(bs->GetMemoryMap,5,&size,map_data,&map_key,&stride,&version);
 check(!EFI_ERROR(s)&&version==EFI_MEMORY_DESCRIPTOR_VERSION&&stride>=sizeof(EFI_MEMORY_DESCRIPTOR)&&size%stride==0,"map");
 count=size/stride;
}
static UINT64 end_of(EFI_MEMORY_DESCRIPTOR *d){
 check(d->NumberOfPages>0&&d->NumberOfPages<=(~d->PhysicalStart)/PAGE,"descriptor bounds");
 return d->PhysicalStart+d->NumberOfPages*PAGE;
}
static void allocate(EFI_BOOT_SERVICES *bs,UINT64 base,UINT64 bytes){
 EFI_PHYSICAL_ADDRESS p=base;
 EFI_STATUS s=uefi_call_wrapper(bs->AllocatePages,4,AllocateAddress,EfiLoaderData,bytes/PAGE,&p);
 check(!EFI_ERROR(s)&&p==base,"allocate");
}
static void claim_low(EFI_BOOT_SERVICES *bs,UINT64 a,UINT64 len){
 get_map(bs);int ok=0;
 for(UINTN i=0;i<count;i++){
   EFI_MEMORY_DESCRIPTOR *e=desc(i);
   if(e->Type==EfiConventionalMemory&&!(e->Attribute&EFI_MEMORY_RUNTIME)&&e->PhysicalStart<=a&&end_of(e)>=a+len)ok=1;
 }
 check(ok,"low area under firmware ownership");allocate(bs,a,len);uefi_call_wrapper(bs->SetMem,3,(void *)(UINTN)a,len,0);
}
static void set_mod(unsigned i,UINT32 addr,UINT32 length,const char *name){
 for(unsigned j=0;j<15&&name[j];j++)boot->names[i][j]=name[j];
 boot->mods[i]=(struct mb_module){addr,addr+length,(UINT32)(UINTN)boot->names[i],0};
}
static UINT64 physical(UINT64 off){return off<FRONT?model+off:second+off-FRONT;}
static UINT64 min64(UINT64 a,UINT64 b){return a<b?a:b;}
static void load_file(EFI_FILE_HANDLE root,CHAR16 *name,UINT64 pos,UINT64 length,UINT8 *direct){
 EFI_FILE_HANDLE f=NULL;
 EFI_STATUS s=uefi_call_wrapper(root->Open,5,root,&f,name,EFI_FILE_MODE_READ,0);
 check(!EFI_ERROR(s)&&f!=NULL,"file open");
 for(UINT64 off=0;off<length;){
  UINT64 logical=pos+off; UINTN want=min64(1<<20,length-off);
  if(!direct&&logical<FRONT)want=min64(want,FRONT-logical);
  UINTN got=want;
  s=uefi_call_wrapper(f->Read,3,f,&got,direct?direct+off:(void *)(UINTN)physical(logical));
  check(!EFI_ERROR(s)&&got==want,"file length");off+=got;
 }
 UINT8 extra;UINTN n=1;s=uefi_call_wrapper(f->Read,3,f,&n,&extra);
 check(!EFI_ERROR(s)&&n==0&&!EFI_ERROR(uefi_call_wrapper(f->Close,1,f)),"EOF/close");
}
static void range(UINT64 a,UINT64 b,UINT32 t){
 check(record_count<512,"multiboot map capacity");
 boot->map[record_count++]=(struct mb_range){20,a,b-a,t};
 msg("range=");hex(a);emit(',');hex(b-a);emit(',');hex(t);emit('\n');
}
static void verify_map(void){
 record_count=0;
 UINT64 marked=0,guard_bytes=0; int low_owned=0;UINTN touches=0;
 for(UINTN i=0;i<count;i++){
  EFI_MEMORY_DESCRIPTOR *d=desc(i);UINT64 a=d->PhysicalStart,b=end_of(d);
  if(a<=LOW_MANIFEST&&b>=LOW_MANIFEST+PAGE){check(d->Type==EfiLoaderData&&!(d->Attribute&EFI_MEMORY_RUNTIME),"low module ownership");low_owned=1;}
  int intersects=a<second+ROUNDED-FRONT&&b>model;
  if(intersects){touches++;check(d->Type==EfiLoaderData&&!(d->Attribute&EFI_MEMORY_RUNTIME),"model not loader RAM");}
  UINT64 stops[]={model,guard,second,second+ROUNDED-FRONT};
  while(a<b){
   UINT64 to=b;
   for(UINTN j=0;j<4;j++)if(stops[j]>a&&stops[j]<to)to=stops[j];
   UINT32 t=d->Type==EfiConventionalMemory?1:(d->Type==EfiACPIMemoryNVS?4:2);
   if((a>=model&&a<guard)||(a>=second&&a<second+ROUNDED-FRONT)){t=MODEL_KIND;marked+=to-a;}
   if(a>=guard&&a<second){check(t==2,"guard classification");guard_bytes+=to-a;}
   range(a,to,t);a=to;
  }
 }
 check(low_owned&&marked==ROUNDED&&guard_bytes==PAGE,"coverage");
 num("raw EFI descriptors covering payload and guard",touches);
}
static void repeated(UINT64 logical,UINT64 n,UINT8 value){
 while(n){
  UINT64 chunk=logical<FRONT?min64(n,FRONT-logical):n;
  const UINT8 *p=(const UINT8 *)(UINTN)physical(logical);
  UINT64 words=chunk/8,pattern=(UINT64)value*0x0101010101010101ULL;
  if(words){unsigned char ok;__asm__ volatile("cld; repe scasq; sete %0":"=qm"(ok),"+D"(p),"+c"(words):"a"(pattern):"cc","memory");check(ok,"payload words");}
  for(UINTN j=0;j<(chunk%8);j++)check(p[j]==value,"payload tail");
  logical+=chunk;n-=chunk;
 }
}
EFI_STATUS efi_main(EFI_HANDLE image,EFI_SYSTEM_TABLE *system){
 unsigned a=0x40000000,b,c,d;
 __asm__ volatile("cpuid":"+a"(a),"=b"(b),"=c"(c),"=d"(d));
 if(b!=0x54474354||c!=0x43544743||d!=0x47435447)return EFI_UNSUPPORTED;
 InitializeLib(image,system);EFI_BOOT_SERVICES *bs=system->BootServices;
 EFI_LOADED_IMAGE *li=NULL;
 check(!EFI_ERROR(uefi_call_wrapper(bs->HandleProtocol,3,image,&LoadedImageProtocol,(void **)&li))&&li,"loaded image");
 EFI_FILE_HANDLE root=LibOpenRoot(li->DeviceHandle);check(root!=NULL,"boot volume");
 get_map(bs);
 for(UINTN i=0;i<count;i++){
  EFI_MEMORY_DESCRIPTOR *e=desc(i);UINT64 end=end_of(e);
  if(e->Type!=EfiConventionalMemory||(e->Attribute&EFI_MEMORY_RUNTIME))continue;
  UINT64 base=e->PhysicalStart>HIGH?e->PhysicalStart:HIGH;
  if(base>~0ULL-(HUGE-1))continue;
  base=(base+HUGE-1)&~(HUGE-1);
  if(end>=base&&end-base>=ROUNDED+PAGE){model=base;break;}
 }
 check(model!=0,"conventional capacity");guard=model+FRONT;second=guard+PAGE;
 int low_free=0;
 for(UINTN i=0;i<count;i++){EFI_MEMORY_DESCRIPTOR *e=desc(i);if(e->Type==EfiConventionalMemory&&!(e->Attribute&EFI_MEMORY_RUNTIME)&&e->PhysicalStart<=LOW_MANIFEST&&end_of(e)>=LOW_MANIFEST+PAGE)low_free=1;}
 check((UINTN)transfer<0x100000000ULL&&(UINTN)transfer_end<0x100000000ULL,"transition address");
 claim_low(bs,LOW_KERNEL,KERNEL_END-LOW_KERNEL);
 claim_low(bs,FRONT_ADDR,(FRONT_FILE+PAGE-1)&~(PAGE-1));claim_low(bs,READER_ADDR,(READER_FILE+PAGE-1)&~(PAGE-1));
 claim_low(bs,BOOT_ADDR,BOOT_LEN);
 check(low_free,"low conventional capacity");allocate(bs,LOW_MANIFEST,PAGE);
 allocate(bs,model,FRONT);allocate(bs,guard,PAGE);allocate(bs,second,ROUNDED-FRONT);
 uefi_call_wrapper(bs->SetMem,3,(void *)(UINTN)guard,PAGE,0xa7);
 for(unsigned i=0;i<PAGE;i++)check(*(volatile UINT8 *)(UINTN)(guard+i)==0xa7,"guard initialization");
 volatile UINT8 *padding=(volatile UINT8 *)(UINTN)physical(TOTAL);
 for(UINTN j=0;j<ROUNDED-TOTAL;j++)padding[j]=0x5a;
 for(UINTN j=0;j<ROUNDED-TOTAL;j++)check(padding[j]==0x5a,"padding initialization");
 uefi_call_wrapper(bs->SetMem,3,(void *)(UINTN)padding,ROUNDED-TOTAL,0);
 num("padding cleared",ROUNDED-TOTAL);
 num("model",model);num("guard",guard);num("second",second);
 UINT8 *low=(UINT8 *)(UINTN)LOW_MANIFEST;
 load_file(root,L"MODEL.MANIFEST",0,sizeof(expected_manifest),low);
 const char *name="model.manifest"; UINTN name_len=0;
 do {low[3072+name_len]=name[name_len];} while(name[name_len++]!=0);
 struct mb_module *mod=(struct mb_module *)(low+3008);
 *mod=(struct mb_module){LOW_MANIFEST,LOW_MANIFEST+sizeof(expected_manifest),LOW_MANIFEST+3072,0};
 char line[260]; char *out=line;
 const char *first="model_memory=";
 while(*first)*out++=*first++;
 const UINT64 parts[]={model,FRONT,second,TOTAL-FRONT};
 for(unsigned i=0;i<4;i++){
  if(i)*out++=i==2?',':':';
  *out++='0';*out++='x';
  for(int bit=60;bit>=0;bit-=4)*out++="0123456789abcdef"[(parts[i]>>bit)&15];
 }
 const char *pin=" model_manifest=" EXPECTED_PIN;
 while(*pin)*out++=*pin++;
 *out=0;
 load_file(root,L"KERNEL.BIN",0,KERNEL_FILE,(UINT8 *)(UINTN)LOW_KERNEL);
 load_file(root,L"FRONT.ELF",0,FRONT_FILE,(UINT8 *)(UINTN)FRONT_ADDR);
 load_file(root,L"READER.ELF",0,READER_FILE,(UINT8 *)(UINTN)READER_ADDR);
 set_mod(0,FRONT_ADDR,FRONT_FILE,"model-front");set_mod(1,READER_ADDR,READER_FILE,"model-reader");set_mod(2,LOW_MANIFEST,sizeof(expected_manifest),"model.manifest");
 boot->info[0]=4|8|0x40;boot->info[4]=(UINTN)boot->argv;boot->info[5]=3;boot->info[6]=(UINTN)boot->mods;boot->info[12]=(UINTN)boot->map;
 const char *pre=LAUNCH_WORDS;UINTN nline=0;
 while(*pre)boot->argv[nline++]=*pre++;
 const char *words=line;
 while(*words){check(nline+1<sizeof(boot->argv),"line capacity");boot->argv[nline++]=*words++;}
 boot->argv[nline]=0;
 UINT64 pos=0;
 load_file(root,L"WEIGHTS.GGUF",0,TOTAL,NULL);pos=TOTAL;
 check(pos==TOTAL&&!EFI_ERROR(uefi_call_wrapper(root->Close,1,root)),"total");
 EFI_STATUS s=EFI_ABORTED;
 for(unsigned attempt=0;attempt<3;attempt++){
  get_map(bs);verify_map();boot->info[11]=record_count*sizeof(struct mb_range);
  s=uefi_call_wrapper(bs->ExitBootServices,2,image,map_key);
  if(s==EFI_SUCCESS)break;
  check(s==EFI_INVALID_PARAMETER,"exit");
 }
 check(s==EFI_SUCCESS,"exit retries");
 check(mod->start==LOW_MANIFEST&&mod->end-mod->start==sizeof(expected_manifest)&&mod->name==LOW_MANIFEST+3072&&mod->reserved==0,"low module descriptor");
 for(UINTN i=0;i<sizeof(expected_manifest);i++)check(((volatile UINT8 *)(UINTN)mod->start)[i]==expected_manifest[i],"manifest identity");
 for(UINTN i=0;i<name_len;i++)check(((volatile char *)(UINTN)mod->name)[i]==name[i],"module name");
 num("module start",mod->start);num("module end",mod->end);
 msg(line);emit('\n');pos=0;
 UINT64 fingerprint=14695981039346656037ULL;
 for(pos=0;pos<TOTAL;pos++)fingerprint=(fingerprint^*(volatile UINT8 *)(UINTN)physical(pos))*1099511628211ULL;
 check(fingerprint==3870424940879124977ULL,"firmware GGUF fingerprint");
 for(unsigned i=0;i<PAGE;i++)check(*(volatile UINT8 *)(UINTN)(guard+i)==0xa7,"guard bytes");
 repeated(TOTAL,ROUNDED-TOTAL,0);num("verified after exit",pos);msg("FIRMWARE VERIFIED; TRANSFER\n");transfer(BOOT_ADDR,KERNEL_ENTRY,BOOT_ADDR+BOOT_LEN);
 return EFI_SUCCESS;
}

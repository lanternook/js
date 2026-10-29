#define UNICODE
#define _UNICODE
#include <windows.h>
#include <winhttp.h>
#include <string>
#include <vector>
#include <fstream>

#pragma comment(lib, "winhttp.lib")

static HWND g_hHost, g_hCid, g_hCsec, g_hName, g_hVal, g_hStatus;
static HFONT g_font = NULL;
static int   g_dpi  = 96;

// 按当前系统 DPI 缩放像素坐标（96dpi 为基准）
static int S(int v){ return MulDiv(v, g_dpi, 96); }

// 取系统界面默认字体（消息框字体，Win10/11 上即 Segoe UI / 微软雅黑 UI）
static void InitDpiAndFont(){
    HDC hdc = GetDC(NULL);
    if(hdc){ int d = GetDeviceCaps(hdc, LOGPIXELSY); if(d>0) g_dpi = d; ReleaseDC(NULL, hdc); }
    NONCLIENTMETRICSW ncm;
    ZeroMemory(&ncm, sizeof(ncm));
    ncm.cbSize = sizeof(ncm);
    if(SystemParametersInfoW(SPI_GETNONCLIENTMETRICS, sizeof(ncm), &ncm, 0))
        g_font = CreateFontIndirectW(&ncm.lfMessageFont);
    if(!g_font) g_font = (HFONT)GetStockObject(DEFAULT_GUI_FONT);
}

static std::wstring A2W(const std::string& s){
    int n = MultiByteToWideChar(CP_UTF8, 0, s.c_str(), -1, nullptr, 0);
    std::wstring w; w.resize(n-1);
    MultiByteToWideChar(CP_UTF8, 0, s.c_str(), -1, &w[0], n);
    return w;
}
static std::string W2A(const std::wstring& w){
    int n = WideCharToMultiByte(CP_UTF8, 0, w.c_str(), -1, nullptr, 0, nullptr, nullptr);
    std::string s; s.resize(n-1);
    WideCharToMultiByte(CP_UTF8, 0, w.c_str(), -1, &s[0], n, nullptr, nullptr);
    return s;
}
static std::wstring GetText(HWND h){
    int n = GetWindowTextLengthW(h);
    std::wstring s; s.resize(n+1);
    GetWindowTextW(h, &s[0], n+1);
    s.resize(n);
    return s;
}
static void SetStatus(const std::wstring& msg){
    if(g_hStatus) SetWindowTextW(g_hStatus, msg.c_str());
}

static std::string jesc(const std::string& s){
    std::string o;
    for(char c: s){
        if(c=='"'||c=='\\'){ o+='\\'; o+=c; }
        else if(c=='\n') o+="\\n";
        else if(c=='\r') o+="\\r";
        else if(c=='\t') o+="\\t";
        else o+=c;
    }
    return o;
}
// JSON 压缩（Minify）：去除字符串外的空白，压成单行。
// 算法参考 simdjson json_minifier / rapidjson condense：状态机跟踪引号与转义，
// 字符串内部空白原样保留，只删外部空白。
static std::string JsonMinify(const std::string& in){
    std::string out; out.reserve(in.size());
    bool inStr=false, esc=false;
    for(char c: in){
        if(inStr){
            out+=c;
            if(esc) esc=false;
            else if(c=='\\') esc=true;
            else if(c=='"') inStr=false;
        }else{
            if(c=='"'){ inStr=true; out+=c; }
            else if(c!=' '&&c!='\t'&&c!='\n'&&c!='\r') out+=c;
        }
    }
    return out;
}

static std::string extractToken(const std::string& b){
    std::string k="\"token\":\"";
    auto p=b.find(k); if(p==std::string::npos) return "";
    p+=k.size(); auto q=b.find('"', p);
    return q==std::string::npos ? std::string() : b.substr(p, q-p);
}
static std::vector<std::string> extractNames(const std::string& b){
    std::vector<std::string> v; std::string k="\"name\":\"";
    size_t p=0;
    while((p=b.find(k,p))!=std::string::npos){
        p+=k.size(); auto q=b.find('"', p);
        if(q==std::string::npos) break;
        v.push_back(b.substr(p, q-p)); p=q+1;
    }
    return v;
}

static std::string HttpDo(const std::wstring& url, const std::string& method,
                          const std::vector<std::pair<std::string,std::string>>& hdr,
                          const std::string& body, int* outCode){
    std::string resp;
    URL_COMPONENTS uc={0}; uc.dwStructSize=sizeof(uc);
    wchar_t host[512]={0}, path[8192]={0};
    uc.lpszHostName=host; uc.dwHostNameLength=512;
    uc.lpszUrlPath=path; uc.dwUrlPathLength=8192;
    if(!WinHttpCrackUrl(url.c_str(),0,0,&uc)) return "";
    bool secure = (uc.nScheme==INTERNET_SCHEME_HTTPS);
    HINTERNET hS = WinHttpOpen(L"ql-env-editor/1.0", WINHTTP_ACCESS_TYPE_DEFAULT_PROXY, WINHTTP_NO_PROXY_NAME, WINHTTP_NO_PROXY_BYPASS, 0);
    if(!hS) return "";
    HINTERNET hC = WinHttpConnect(hS, host, uc.nPort, 0);
    if(!hC){ WinHttpCloseHandle(hS); return ""; }
    HINTERNET hR = WinHttpOpenRequest(hC, A2W(method).c_str(), path, NULL, NULL, NULL, secure?WINHTTP_FLAG_SECURE:0);
    if(!hR){ WinHttpCloseHandle(hC); WinHttpCloseHandle(hS); return ""; }
    if(secure){
        DWORD f = SECURITY_FLAG_IGNORE_UNKNOWN_CA|SECURITY_FLAG_IGNORE_CERT_DATE_INVALID|SECURITY_FLAG_IGNORE_CERT_CN_INVALID;
        WinHttpSetOption(hR, WINHTTP_OPTION_SECURITY_FLAGS, &f, sizeof(f));
    }
    std::wstring rh;
    for(auto& kv: hdr){ rh += A2W(kv.first)+L": "+A2W(kv.second)+L"\r\n"; }
    if(!WinHttpSendRequest(hR, rh.c_str(), (DWORD)rh.size(), (LPVOID)body.data(), (DWORD)body.size(), (DWORD)body.size(), 0)){
        WinHttpCloseHandle(hR); WinHttpCloseHandle(hC); WinHttpCloseHandle(hS); return "";
    }
    if(!WinHttpReceiveResponse(hR, NULL)){ WinHttpCloseHandle(hR); WinHttpCloseHandle(hC); WinHttpCloseHandle(hS); return ""; }
    if(outCode){
        DWORD c=0, l=sizeof(c);
        WinHttpQueryHeaders(hR, WINHTTP_QUERY_STATUS_CODE|WINHTTP_QUERY_FLAG_NUMBER, NULL, &c, &l, NULL);
        *outCode=(int)c;
    }
    DWORD avail, read;
    do{
        avail=0;
        if(!WinHttpQueryDataAvailable(hR, &avail)) break;
        if(avail==0) break;
        std::string buf(avail, 0);
        if(!WinHttpReadData(hR, &buf[0], avail, &read)) break;
        buf.resize(read); resp+=buf;
    }while(avail>0);
    WinHttpCloseHandle(hR); WinHttpCloseHandle(hC); WinHttpCloseHandle(hS);
    return resp;
}

static std::wstring ExeDir(){
    wchar_t buf[1024]; GetModuleFileNameW(NULL,buf,1024);
    std::wstring p=buf; auto s=p.find_last_of(L'\\');
    return p.substr(0, s+1);
}
static void SaveSettings(){
    std::wstring f = ExeDir()+L"settings.txt";
    std::ofstream of(W2A(f).c_str());
    of << W2A(GetText(g_hHost)) << "\n" << W2A(GetText(g_hCid)) << "\n" << W2A(GetText(g_hCsec)) << "\n";
    SetStatus(L"设置已保存到 settings.txt");
}
static void LoadSettings(){
    std::wstring f = ExeDir()+L"settings.txt";
    std::ifstream inf(W2A(f).c_str());
    if(!inf) return;
    std::string a,b,c; std::getline(inf,a); std::getline(inf,b); std::getline(inf,c);
    if(!a.empty()) SetWindowTextW(g_hHost, A2W(a).c_str());
    if(!b.empty()) SetWindowTextW(g_hCid, A2W(b).c_str());
    if(!c.empty()) SetWindowTextW(g_hCsec, A2W(c).c_str());
}

static void LoadNames(){
    std::wstring base = GetText(g_hHost);
    if(base.empty()){ SetStatus(L"请先填写青龙地址并保存设置"); return; }
    std::wstring auth = base + L"/open/auth/token?client_id=" + GetText(g_hCid) + L"&client_secret=" + GetText(g_hCsec);
    int code=0;
    std::string r = HttpDo(auth, "GET", {}, "", &code);
    if(r.empty()){ SetStatus(L"获取 token 失败（地址/网络错误）"); return; }
    std::string tok = extractToken(r);
    if(tok.empty()){ SetStatus(A2W("获取 token 失败，返回: "+r.substr(0,200))); return; }
    std::vector<std::pair<std::string,std::string>> h; h.push_back({"Authorization","Bearer "+tok});
    std::string re = HttpDo(base + L"/open/envs", "GET", h, "", &code);
    if(re.empty()){ SetStatus(L"读取环境变量失败"); return; }
    auto names = extractNames(re);
    SendMessageW(g_hName, CB_RESETCONTENT, 0, 0);
    for(auto& n: names) SendMessageW(g_hName, CB_ADDSTRING, 0, (LPARAM)A2W(n).c_str());
    SetStatus(L"已读取 "+std::to_wstring(names.size())+L" 个环境变量名，可在下拉框选择");
}

static void DoUpdate(){
    std::wstring name = GetText(g_hName);
    std::wstring val = GetText(g_hVal);
    if(name.empty()||val.empty()){ SetStatus(L"请选择/填写名称并填写值"); return; }
    std::wstring base = GetText(g_hHost);
    int code=0;
    std::string tok = extractToken(HttpDo(base + L"/open/auth/token?client_id=" + GetText(g_hCid) + L"&client_secret=" + GetText(g_hCsec), "GET", {}, "", &code));
    if(tok.empty()){ SetStatus(L"获取 token 失败，无法更新"); return; }
    std::vector<std::pair<std::string,std::string>> h; h.push_back({"Authorization","Bearer "+tok}); h.push_back({"Content-Type","application/json"});
    std::string re = HttpDo(base + L"/open/envs?searchValue=" + name, "GET", h, "", &code);
    std::string na = W2A(name), needle = "\"name\":\""+na+"\"";
    size_t pos = re.find(needle);
    std::string idv, curval;
    if(pos!=std::string::npos){
        std::string idk="\"id\":";
        auto ip = re.rfind(idk, pos);
        if(ip!=std::string::npos){ ip+=idk.size();
            while(ip<re.size() && (re[ip]==' '||re[ip]=='\t')) ip++;
            auto iq=ip; while(iq<re.size() && re[iq]!=','&&re[iq]!='}'&&re[iq]!=' ') iq++;
            idv=re.substr(ip, iq-ip);
        }
        auto vp = re.find("\"value\":\"", pos);
        if(vp!=std::string::npos){ vp+=9; curval=re.substr(vp, re.find('"', vp)-vp); }
    }
    if(idv.empty()){ SetStatus(A2W("未找到环境变量: "+na+"（请先点读取变量名）")); return; }
    std::string newval = W2A(val), body;
    if(!curval.empty() && curval.find("{")==0 && curval.find("\"sessionCookie\":\"")!=std::string::npos){
        std::string k="\"sessionCookie\":\"";
        auto sp=curval.find(k)+k.size(); auto sq=curval.find('"', sp);
        std::string updated = curval.substr(0, sp) + jesc(newval) + curval.substr(sq);
        body = "{\"id\":"+idv+",\"name\":\""+jesc(na)+"\",\"value\":"+updated+"}";
    } else {
        body = "{\"id\":"+idv+",\"name\":\""+jesc(na)+"\",\"value\":\""+jesc(newval)+"\"}";
    }
    std::string rp = HttpDo(base + L"/open/envs", "PUT", h, body, &code);
    if(code==200) SetStatus(L"更新成功! "+A2W(rp.substr(0,160)));
    else SetStatus(A2W("更新失败 HTTP "+std::to_string(code)+": "+rp.substr(0,160)));
}

#define IDC_HOST 1001
#define IDC_CID 1002
#define IDC_CSEC 1003
#define IDC_SAVE 1004
#define IDC_LOAD 1005
#define IDC_NAME 1006
#define IDC_VAL 1007
#define IDC_UPDATE 1008
#define IDC_STATUS 1009
#define IDC_MINIFY 1010

static LRESULT CALLBACK WndProc(HWND hwnd, UINT msg, WPARAM wp, LPARAM lp){
    switch(msg){
        case WM_CREATE: {
            auto mk = [&](const wchar_t* cls, const wchar_t* txt, DWORD style,
                          int x, int y, int w, int h, int id)->HWND{
                HWND c = CreateWindowExW(0, cls, txt, WS_CHILD|WS_VISIBLE|style,
                                         S(x), S(y), S(w), S(h), hwnd,
                                         (HMENU)(INT_PTR)id, NULL, NULL);
                if(c && g_font) SendMessageW(c, WM_SETFONT, (WPARAM)g_font, TRUE);
                return c;
            };
            mk(L"static", L"青龙地址（含 http:// 和端口，如 http://192.168.50.43:5700）",
               SS_LEFT, 14, 10, 442, 16, 0);
            g_hHost = mk(L"edit", L"", WS_BORDER|ES_AUTOHSCROLL, 14, 30, 442, 24, IDC_HOST);

            mk(L"static", L"Client ID:", SS_LEFT, 14, 72, 200, 16, 0);
            g_hCid = mk(L"edit", L"", WS_BORDER|ES_AUTOHSCROLL, 14, 92, 300, 24, IDC_CID);
            mk(L"button", L"保存设置", BS_PUSHBUTTON, 326, 92, 130, 24, IDC_SAVE);

            mk(L"static", L"Client Secret:", SS_LEFT, 14, 128, 200, 16, 0);
            g_hCsec = mk(L"edit", L"", WS_BORDER|ES_PASSWORD|ES_AUTOHSCROLL, 14, 148, 300, 24, IDC_CSEC);
            mk(L"button", L"读取变量名", BS_PUSHBUTTON, 326, 148, 130, 24, IDC_LOAD);

            mk(L"static", L"环境变量名称（从青龙读取，可下拉选择或手动输入）:", SS_LEFT, 14, 184, 442, 16, 0);
            g_hName = mk(L"combobox", L"", CBS_DROPDOWN|CBS_AUTOHSCROLL|WS_VSCROLL, 14, 204, 442, 180, IDC_NAME);

            mk(L"static", L"值（手动填入，粘贴完整 cookie）:", SS_LEFT, 14, 240, 442, 16, 0);
            g_hVal = mk(L"edit", L"", WS_BORDER|ES_MULTILINE|ES_AUTOVSCROLL|WS_VSCROLL, 14, 260, 442, 84, IDC_VAL);

            mk(L"button", L"确认更新到青龙", BS_PUSHBUTTON|BS_DEFPUSHBUTTON, 14, 356, 170, 28, IDC_UPDATE);
            mk(L"button", L"JSON压缩(Minify)", BS_PUSHBUTTON, 200, 356, 170, 28, IDC_MINIFY);

            g_hStatus = mk(L"edit", L"", WS_BORDER|ES_MULTILINE|ES_READONLY|WS_VSCROLL, 14, 396, 442, 72, IDC_STATUS);

            LoadSettings();
            SetStatus(L"填写青龙地址与凭证 → 保存设置 → 读取变量名 → 选择名称 → 粘贴 cookie → 确认更新");
            break;
        }
        case WM_COMMAND: {
            int id=LOWORD(wp);
            if(id==IDC_SAVE) SaveSettings();
            else if(id==IDC_LOAD) LoadNames();
            else if(id==IDC_UPDATE) DoUpdate();
            else if(id==IDC_MINIFY){
                std::wstring val = GetText(g_hVal);
                if(val.empty()){ SetStatus(L"值框为空，无可压缩内容"); break; }
                std::string m = JsonMinify(W2A(val));
                if(m == W2A(val)){ SetStatus(L"内容已是压缩格式，无需处理"); break; }
                size_t before = W2A(val).size(), after = m.size();
                SetWindowTextW(g_hVal, A2W(m).c_str());
                SetStatus(A2W("JSON压缩完成: "+std::to_string(before)+" -> "+std::to_string(after)+" 字节（单行）"));
            }
            break;
        }
        case WM_DESTROY: PostQuitMessage(0); break;
        default: return DefWindowProcW(hwnd,msg,wp,lp);
    }
    return 0;
}

int WINAPI WinMain(HINSTANCE h, HINSTANCE, LPSTR, int){
    SetProcessDPIAware();
    InitDpiAndFont();

    HICON hBig = (HICON)LoadImageW(h, MAKEINTRESOURCEW(101), IMAGE_ICON,
                                   GetSystemMetrics(SM_CXICON), GetSystemMetrics(SM_CYICON), LR_SHARED);
    HICON hSml = (HICON)LoadImageW(h, MAKEINTRESOURCEW(101), IMAGE_ICON,
                                   GetSystemMetrics(SM_CXSMICON), GetSystemMetrics(SM_CYSMICON), LR_SHARED);
    if(!hBig) hBig = LoadIconW(NULL, IDI_APPLICATION);
    if(!hSml) hSml = LoadIconW(NULL, IDI_APPLICATION);

    WNDCLASSW wc={0};
    wc.lpfnWndProc=WndProc; wc.hInstance=h; wc.lpszClassName=L"QLEditor";
    wc.hCursor=LoadCursor(NULL, IDC_ARROW);
    wc.hbrBackground=(HBRUSH)(COLOR_BTNFACE+1);
    wc.hIcon=hBig;
    RegisterClassW(&wc);

    // 客户区 470x478，按 DPI 与边框自动换算窗口尺寸
    RECT rc={0,0,S(470),S(478)};
    AdjustWindowRectEx(&rc, WS_OVERLAPPEDWINDOW, FALSE, 0);
    HWND w=CreateWindowW(L"QLEditor", L"青龙环境变量更新工具", WS_OVERLAPPEDWINDOW,
                         CW_USEDEFAULT, CW_USEDEFAULT,
                         rc.right-rc.left, rc.bottom-rc.top, NULL, NULL, h, NULL);
    SendMessageW(w, WM_SETICON, ICON_BIG, (LPARAM)hBig);
    SendMessageW(w, WM_SETICON, ICON_SMALL, (LPARAM)hSml);
    ShowWindow(w, SW_SHOW);
    UpdateWindow(w);
    MSG m; while(GetMessageW(&m, NULL, 0, 0)){ TranslateMessage(&m); DispatchMessageW(&m); }
    return 0;
}

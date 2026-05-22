# The Dispatch Benchmark
#
# Copied from GraalVM
# Based on the JavaScript implementation at
# https://github.com/oracle/graal/blob/master/vm/benchmarks/compiler/dispatch.js

import math

var funcs = [
  @(a) {
    var x = a[0]
    var y = a[-1]
    a[0] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1]
    var y = a[-2]
    a[1] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[2]
    var y = a[-3]
    a[2] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[3]
    var y = a[-4]
    a[3] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[4]
    var y = a[-5]
    a[4] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[5]
    var y = a[-6]
    a[5] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[6]
    var y = a[-7]
    a[6] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[7]
    var y = a[-8]
    a[7] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[8]
    var y = a[-9]
    a[8] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[9]
    var y = a[-10]
    a[9] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[10]
    var y = a[-11]
    a[10] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[11]
    var y = a[-12]
    a[11] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[12]
    var y = a[-13]
    a[12] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[13]
    var y = a[-14]
    a[13] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[14]
    var y = a[-15]
    a[14] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[15]
    var y = a[-16]
    a[15] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[16]
    var y = a[-17]
    a[16] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[17]
    var y = a[-18]
    a[17] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[18]
    var y = a[-19]
    a[18] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[19]
    var y = a[-20]
    a[19] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[20]
    var y = a[-21]
    a[20] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[21]
    var y = a[-22]
    a[21] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[22]
    var y = a[-23]
    a[22] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[23]
    var y = a[-24]
    a[23] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[24]
    var y = a[-25]
    a[24] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[25]
    var y = a[-26]
    a[25] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[26]
    var y = a[-27]
    a[26] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[27]
    var y = a[-28]
    a[27] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[28]
    var y = a[-29]
    a[28] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[29]
    var y = a[-30]
    a[29] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[30]
    var y = a[-31]
    a[30] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[31]
    var y = a[-32]
    a[31] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[32]
    var y = a[-33]
    a[32] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[33]
    var y = a[-34]
    a[33] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[34]
    var y = a[-35]
    a[34] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[35]
    var y = a[-36]
    a[35] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[36]
    var y = a[-37]
    a[36] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[37]
    var y = a[-38]
    a[37] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[38]
    var y = a[-39]
    a[38] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[39]
    var y = a[-40]
    a[39] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[40]
    var y = a[-41]
    a[40] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[41]
    var y = a[-42]
    a[41] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[42]
    var y = a[-43]
    a[42] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[43]
    var y = a[-44]
    a[43] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[44]
    var y = a[-45]
    a[44] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[45]
    var y = a[-46]
    a[45] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[46]
    var y = a[-47]
    a[46] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[47]
    var y = a[-48]
    a[47] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[48]
    var y = a[-49]
    a[48] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[49]
    var y = a[-50]
    a[49] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[50]
    var y = a[-51]
    a[50] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[51]
    var y = a[-52]
    a[51] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[52]
    var y = a[-53]
    a[52] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[53]
    var y = a[-54]
    a[53] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[54]
    var y = a[-55]
    a[54] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[55]
    var y = a[-56]
    a[55] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[56]
    var y = a[-57]
    a[56] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[57]
    var y = a[-58]
    a[57] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[58]
    var y = a[-59]
    a[58] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[59]
    var y = a[-60]
    a[59] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[60]
    var y = a[-61]
    a[60] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[61]
    var y = a[-62]
    a[61] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[62]
    var y = a[-63]
    a[62] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[63]
    var y = a[-64]
    a[63] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[64]
    var y = a[-65]
    a[64] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[65]
    var y = a[-66]
    a[65] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[66]
    var y = a[-67]
    a[66] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[67]
    var y = a[-68]
    a[67] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[68]
    var y = a[-69]
    a[68] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[69]
    var y = a[-70]
    a[69] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[70]
    var y = a[-71]
    a[70] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[71]
    var y = a[-72]
    a[71] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[72]
    var y = a[-73]
    a[72] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[73]
    var y = a[-74]
    a[73] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[74]
    var y = a[-75]
    a[74] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[75]
    var y = a[-76]
    a[75] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[76]
    var y = a[-77]
    a[76] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[77]
    var y = a[-78]
    a[77] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[78]
    var y = a[-79]
    a[78] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[79]
    var y = a[-80]
    a[79] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[80]
    var y = a[-81]
    a[80] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[81]
    var y = a[-82]
    a[81] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[82]
    var y = a[-83]
    a[82] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[83]
    var y = a[-84]
    a[83] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[84]
    var y = a[-85]
    a[84] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[85]
    var y = a[-86]
    a[85] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[86]
    var y = a[-87]
    a[86] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[87]
    var y = a[-88]
    a[87] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[88]
    var y = a[-89]
    a[88] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[89]
    var y = a[-90]
    a[89] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[90]
    var y = a[-91]
    a[90] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[91]
    var y = a[-92]
    a[91] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[92]
    var y = a[-93]
    a[92] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[93]
    var y = a[-94]
    a[93] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[94]
    var y = a[-95]
    a[94] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[95]
    var y = a[-96]
    a[95] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[96]
    var y = a[-97]
    a[96] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[97]
    var y = a[-98]
    a[97] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[98]
    var y = a[-99]
    a[98] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[99]
    var y = a[-100]
    a[99] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[100]
    var y = a[-101]
    a[100] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[101]
    var y = a[-102]
    a[101] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[102]
    var y = a[-103]
    a[102] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[103]
    var y = a[-104]
    a[103] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[104]
    var y = a[-105]
    a[104] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[105]
    var y = a[-106]
    a[105] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[106]
    var y = a[-107]
    a[106] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[107]
    var y = a[-108]
    a[107] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[108]
    var y = a[-109]
    a[108] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[109]
    var y = a[-110]
    a[109] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[110]
    var y = a[-111]
    a[110] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[111]
    var y = a[-112]
    a[111] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[112]
    var y = a[-113]
    a[112] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[113]
    var y = a[-114]
    a[113] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[114]
    var y = a[-115]
    a[114] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[115]
    var y = a[-116]
    a[115] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[116]
    var y = a[-117]
    a[116] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[117]
    var y = a[-118]
    a[117] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[118]
    var y = a[-119]
    a[118] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[119]
    var y = a[-120]
    a[119] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[120]
    var y = a[-121]
    a[120] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[121]
    var y = a[-122]
    a[121] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[122]
    var y = a[-123]
    a[122] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[123]
    var y = a[-124]
    a[123] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[124]
    var y = a[-125]
    a[124] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[125]
    var y = a[-126]
    a[125] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[126]
    var y = a[-127]
    a[126] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[127]
    var y = a[-128]
    a[127] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[128]
    var y = a[-129]
    a[128] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[129]
    var y = a[-130]
    a[129] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[130]
    var y = a[-131]
    a[130] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[131]
    var y = a[-132]
    a[131] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[132]
    var y = a[-133]
    a[132] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[133]
    var y = a[-134]
    a[133] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[134]
    var y = a[-135]
    a[134] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[135]
    var y = a[-136]
    a[135] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[136]
    var y = a[-137]
    a[136] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[137]
    var y = a[-138]
    a[137] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[138]
    var y = a[-139]
    a[138] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[139]
    var y = a[-140]
    a[139] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[140]
    var y = a[-141]
    a[140] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[141]
    var y = a[-142]
    a[141] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[142]
    var y = a[-143]
    a[142] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[143]
    var y = a[-144]
    a[143] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[144]
    var y = a[-145]
    a[144] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[145]
    var y = a[-146]
    a[145] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[146]
    var y = a[-147]
    a[146] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[147]
    var y = a[-148]
    a[147] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[148]
    var y = a[-149]
    a[148] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[149]
    var y = a[-150]
    a[149] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[150]
    var y = a[-151]
    a[150] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[151]
    var y = a[-152]
    a[151] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[152]
    var y = a[-153]
    a[152] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[153]
    var y = a[-154]
    a[153] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[154]
    var y = a[-155]
    a[154] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[155]
    var y = a[-156]
    a[155] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[156]
    var y = a[-157]
    a[156] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[157]
    var y = a[-158]
    a[157] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[158]
    var y = a[-159]
    a[158] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[159]
    var y = a[-160]
    a[159] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[160]
    var y = a[-161]
    a[160] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[161]
    var y = a[-162]
    a[161] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[162]
    var y = a[-163]
    a[162] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[163]
    var y = a[-164]
    a[163] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[164]
    var y = a[-165]
    a[164] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[165]
    var y = a[-166]
    a[165] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[166]
    var y = a[-167]
    a[166] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[167]
    var y = a[-168]
    a[167] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[168]
    var y = a[-169]
    a[168] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[169]
    var y = a[-170]
    a[169] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[170]
    var y = a[-171]
    a[170] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[171]
    var y = a[-172]
    a[171] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[172]
    var y = a[-173]
    a[172] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[173]
    var y = a[-174]
    a[173] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[174]
    var y = a[-175]
    a[174] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[175]
    var y = a[-176]
    a[175] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[176]
    var y = a[-177]
    a[176] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[177]
    var y = a[-178]
    a[177] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[178]
    var y = a[-179]
    a[178] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[179]
    var y = a[-180]
    a[179] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[180]
    var y = a[-181]
    a[180] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[181]
    var y = a[-182]
    a[181] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[182]
    var y = a[-183]
    a[182] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[183]
    var y = a[-184]
    a[183] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[184]
    var y = a[-185]
    a[184] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[185]
    var y = a[-186]
    a[185] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[186]
    var y = a[-187]
    a[186] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[187]
    var y = a[-188]
    a[187] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[188]
    var y = a[-189]
    a[188] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[189]
    var y = a[-190]
    a[189] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[190]
    var y = a[-191]
    a[190] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[191]
    var y = a[-192]
    a[191] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[192]
    var y = a[-193]
    a[192] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[193]
    var y = a[-194]
    a[193] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[194]
    var y = a[-195]
    a[194] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[195]
    var y = a[-196]
    a[195] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[196]
    var y = a[-197]
    a[196] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[197]
    var y = a[-198]
    a[197] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[198]
    var y = a[-199]
    a[198] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[199]
    var y = a[-200]
    a[199] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[200]
    var y = a[-201]
    a[200] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[201]
    var y = a[-202]
    a[201] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[202]
    var y = a[-203]
    a[202] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[203]
    var y = a[-204]
    a[203] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[204]
    var y = a[-205]
    a[204] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[205]
    var y = a[-206]
    a[205] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[206]
    var y = a[-207]
    a[206] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[207]
    var y = a[-208]
    a[207] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[208]
    var y = a[-209]
    a[208] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[209]
    var y = a[-210]
    a[209] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[210]
    var y = a[-211]
    a[210] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[211]
    var y = a[-212]
    a[211] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[212]
    var y = a[-213]
    a[212] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[213]
    var y = a[-214]
    a[213] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[214]
    var y = a[-215]
    a[214] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[215]
    var y = a[-216]
    a[215] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[216]
    var y = a[-217]
    a[216] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[217]
    var y = a[-218]
    a[217] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[218]
    var y = a[-219]
    a[218] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[219]
    var y = a[-220]
    a[219] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[220]
    var y = a[-221]
    a[220] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[221]
    var y = a[-222]
    a[221] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[222]
    var y = a[-223]
    a[222] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[223]
    var y = a[-224]
    a[223] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[224]
    var y = a[-225]
    a[224] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[225]
    var y = a[-226]
    a[225] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[226]
    var y = a[-227]
    a[226] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[227]
    var y = a[-228]
    a[227] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[228]
    var y = a[-229]
    a[228] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[229]
    var y = a[-230]
    a[229] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[230]
    var y = a[-231]
    a[230] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[231]
    var y = a[-232]
    a[231] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[232]
    var y = a[-233]
    a[232] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[233]
    var y = a[-234]
    a[233] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[234]
    var y = a[-235]
    a[234] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[235]
    var y = a[-236]
    a[235] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[236]
    var y = a[-237]
    a[236] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[237]
    var y = a[-238]
    a[237] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[238]
    var y = a[-239]
    a[238] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[239]
    var y = a[-240]
    a[239] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[240]
    var y = a[-241]
    a[240] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[241]
    var y = a[-242]
    a[241] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[242]
    var y = a[-243]
    a[242] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[243]
    var y = a[-244]
    a[243] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[244]
    var y = a[-245]
    a[244] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[245]
    var y = a[-246]
    a[245] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[246]
    var y = a[-247]
    a[246] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[247]
    var y = a[-248]
    a[247] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[248]
    var y = a[-249]
    a[248] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[249]
    var y = a[-250]
    a[249] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[250]
    var y = a[-251]
    a[250] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[251]
    var y = a[-252]
    a[251] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[252]
    var y = a[-253]
    a[252] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[253]
    var y = a[-254]
    a[253] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[254]
    var y = a[-255]
    a[254] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[255]
    var y = a[-256]
    a[255] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[256]
    var y = a[-257]
    a[256] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[257]
    var y = a[-258]
    a[257] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[258]
    var y = a[-259]
    a[258] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[259]
    var y = a[-260]
    a[259] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[260]
    var y = a[-261]
    a[260] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[261]
    var y = a[-262]
    a[261] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[262]
    var y = a[-263]
    a[262] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[263]
    var y = a[-264]
    a[263] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[264]
    var y = a[-265]
    a[264] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[265]
    var y = a[-266]
    a[265] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[266]
    var y = a[-267]
    a[266] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[267]
    var y = a[-268]
    a[267] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[268]
    var y = a[-269]
    a[268] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[269]
    var y = a[-270]
    a[269] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[270]
    var y = a[-271]
    a[270] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[271]
    var y = a[-272]
    a[271] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[272]
    var y = a[-273]
    a[272] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[273]
    var y = a[-274]
    a[273] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[274]
    var y = a[-275]
    a[274] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[275]
    var y = a[-276]
    a[275] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[276]
    var y = a[-277]
    a[276] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[277]
    var y = a[-278]
    a[277] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[278]
    var y = a[-279]
    a[278] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[279]
    var y = a[-280]
    a[279] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[280]
    var y = a[-281]
    a[280] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[281]
    var y = a[-282]
    a[281] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[282]
    var y = a[-283]
    a[282] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[283]
    var y = a[-284]
    a[283] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[284]
    var y = a[-285]
    a[284] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[285]
    var y = a[-286]
    a[285] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[286]
    var y = a[-287]
    a[286] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[287]
    var y = a[-288]
    a[287] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[288]
    var y = a[-289]
    a[288] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[289]
    var y = a[-290]
    a[289] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[290]
    var y = a[-291]
    a[290] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[291]
    var y = a[-292]
    a[291] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[292]
    var y = a[-293]
    a[292] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[293]
    var y = a[-294]
    a[293] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[294]
    var y = a[-295]
    a[294] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[295]
    var y = a[-296]
    a[295] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[296]
    var y = a[-297]
    a[296] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[297]
    var y = a[-298]
    a[297] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[298]
    var y = a[-299]
    a[298] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[299]
    var y = a[-300]
    a[299] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[300]
    var y = a[-301]
    a[300] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[301]
    var y = a[-302]
    a[301] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[302]
    var y = a[-303]
    a[302] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[303]
    var y = a[-304]
    a[303] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[304]
    var y = a[-305]
    a[304] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[305]
    var y = a[-306]
    a[305] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[306]
    var y = a[-307]
    a[306] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[307]
    var y = a[-308]
    a[307] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[308]
    var y = a[-309]
    a[308] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[309]
    var y = a[-310]
    a[309] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[310]
    var y = a[-311]
    a[310] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[311]
    var y = a[-312]
    a[311] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[312]
    var y = a[-313]
    a[312] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[313]
    var y = a[-314]
    a[313] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[314]
    var y = a[-315]
    a[314] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[315]
    var y = a[-316]
    a[315] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[316]
    var y = a[-317]
    a[316] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[317]
    var y = a[-318]
    a[317] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[318]
    var y = a[-319]
    a[318] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[319]
    var y = a[-320]
    a[319] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[320]
    var y = a[-321]
    a[320] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[321]
    var y = a[-322]
    a[321] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[322]
    var y = a[-323]
    a[322] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[323]
    var y = a[-324]
    a[323] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[324]
    var y = a[-325]
    a[324] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[325]
    var y = a[-326]
    a[325] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[326]
    var y = a[-327]
    a[326] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[327]
    var y = a[-328]
    a[327] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[328]
    var y = a[-329]
    a[328] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[329]
    var y = a[-330]
    a[329] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[330]
    var y = a[-331]
    a[330] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[331]
    var y = a[-332]
    a[331] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[332]
    var y = a[-333]
    a[332] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[333]
    var y = a[-334]
    a[333] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[334]
    var y = a[-335]
    a[334] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[335]
    var y = a[-336]
    a[335] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[336]
    var y = a[-337]
    a[336] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[337]
    var y = a[-338]
    a[337] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[338]
    var y = a[-339]
    a[338] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[339]
    var y = a[-340]
    a[339] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[340]
    var y = a[-341]
    a[340] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[341]
    var y = a[-342]
    a[341] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[342]
    var y = a[-343]
    a[342] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[343]
    var y = a[-344]
    a[343] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[344]
    var y = a[-345]
    a[344] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[345]
    var y = a[-346]
    a[345] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[346]
    var y = a[-347]
    a[346] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[347]
    var y = a[-348]
    a[347] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[348]
    var y = a[-349]
    a[348] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[349]
    var y = a[-350]
    a[349] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[350]
    var y = a[-351]
    a[350] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[351]
    var y = a[-352]
    a[351] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[352]
    var y = a[-353]
    a[352] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[353]
    var y = a[-354]
    a[353] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[354]
    var y = a[-355]
    a[354] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[355]
    var y = a[-356]
    a[355] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[356]
    var y = a[-357]
    a[356] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[357]
    var y = a[-358]
    a[357] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[358]
    var y = a[-359]
    a[358] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[359]
    var y = a[-360]
    a[359] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[360]
    var y = a[-361]
    a[360] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[361]
    var y = a[-362]
    a[361] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[362]
    var y = a[-363]
    a[362] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[363]
    var y = a[-364]
    a[363] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[364]
    var y = a[-365]
    a[364] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[365]
    var y = a[-366]
    a[365] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[366]
    var y = a[-367]
    a[366] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[367]
    var y = a[-368]
    a[367] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[368]
    var y = a[-369]
    a[368] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[369]
    var y = a[-370]
    a[369] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[370]
    var y = a[-371]
    a[370] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[371]
    var y = a[-372]
    a[371] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[372]
    var y = a[-373]
    a[372] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[373]
    var y = a[-374]
    a[373] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[374]
    var y = a[-375]
    a[374] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[375]
    var y = a[-376]
    a[375] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[376]
    var y = a[-377]
    a[376] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[377]
    var y = a[-378]
    a[377] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[378]
    var y = a[-379]
    a[378] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[379]
    var y = a[-380]
    a[379] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[380]
    var y = a[-381]
    a[380] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[381]
    var y = a[-382]
    a[381] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[382]
    var y = a[-383]
    a[382] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[383]
    var y = a[-384]
    a[383] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[384]
    var y = a[-385]
    a[384] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[385]
    var y = a[-386]
    a[385] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[386]
    var y = a[-387]
    a[386] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[387]
    var y = a[-388]
    a[387] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[388]
    var y = a[-389]
    a[388] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[389]
    var y = a[-390]
    a[389] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[390]
    var y = a[-391]
    a[390] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[391]
    var y = a[-392]
    a[391] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[392]
    var y = a[-393]
    a[392] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[393]
    var y = a[-394]
    a[393] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[394]
    var y = a[-395]
    a[394] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[395]
    var y = a[-396]
    a[395] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[396]
    var y = a[-397]
    a[396] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[397]
    var y = a[-398]
    a[397] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[398]
    var y = a[-399]
    a[398] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[399]
    var y = a[-400]
    a[399] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[400]
    var y = a[-401]
    a[400] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[401]
    var y = a[-402]
    a[401] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[402]
    var y = a[-403]
    a[402] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[403]
    var y = a[-404]
    a[403] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[404]
    var y = a[-405]
    a[404] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[405]
    var y = a[-406]
    a[405] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[406]
    var y = a[-407]
    a[406] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[407]
    var y = a[-408]
    a[407] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[408]
    var y = a[-409]
    a[408] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[409]
    var y = a[-410]
    a[409] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[410]
    var y = a[-411]
    a[410] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[411]
    var y = a[-412]
    a[411] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[412]
    var y = a[-413]
    a[412] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[413]
    var y = a[-414]
    a[413] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[414]
    var y = a[-415]
    a[414] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[415]
    var y = a[-416]
    a[415] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[416]
    var y = a[-417]
    a[416] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[417]
    var y = a[-418]
    a[417] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[418]
    var y = a[-419]
    a[418] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[419]
    var y = a[-420]
    a[419] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[420]
    var y = a[-421]
    a[420] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[421]
    var y = a[-422]
    a[421] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[422]
    var y = a[-423]
    a[422] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[423]
    var y = a[-424]
    a[423] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[424]
    var y = a[-425]
    a[424] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[425]
    var y = a[-426]
    a[425] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[426]
    var y = a[-427]
    a[426] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[427]
    var y = a[-428]
    a[427] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[428]
    var y = a[-429]
    a[428] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[429]
    var y = a[-430]
    a[429] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[430]
    var y = a[-431]
    a[430] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[431]
    var y = a[-432]
    a[431] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[432]
    var y = a[-433]
    a[432] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[433]
    var y = a[-434]
    a[433] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[434]
    var y = a[-435]
    a[434] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[435]
    var y = a[-436]
    a[435] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[436]
    var y = a[-437]
    a[436] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[437]
    var y = a[-438]
    a[437] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[438]
    var y = a[-439]
    a[438] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[439]
    var y = a[-440]
    a[439] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[440]
    var y = a[-441]
    a[440] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[441]
    var y = a[-442]
    a[441] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[442]
    var y = a[-443]
    a[442] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[443]
    var y = a[-444]
    a[443] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[444]
    var y = a[-445]
    a[444] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[445]
    var y = a[-446]
    a[445] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[446]
    var y = a[-447]
    a[446] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[447]
    var y = a[-448]
    a[447] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[448]
    var y = a[-449]
    a[448] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[449]
    var y = a[-450]
    a[449] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[450]
    var y = a[-451]
    a[450] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[451]
    var y = a[-452]
    a[451] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[452]
    var y = a[-453]
    a[452] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[453]
    var y = a[-454]
    a[453] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[454]
    var y = a[-455]
    a[454] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[455]
    var y = a[-456]
    a[455] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[456]
    var y = a[-457]
    a[456] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[457]
    var y = a[-458]
    a[457] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[458]
    var y = a[-459]
    a[458] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[459]
    var y = a[-460]
    a[459] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[460]
    var y = a[-461]
    a[460] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[461]
    var y = a[-462]
    a[461] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[462]
    var y = a[-463]
    a[462] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[463]
    var y = a[-464]
    a[463] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[464]
    var y = a[-465]
    a[464] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[465]
    var y = a[-466]
    a[465] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[466]
    var y = a[-467]
    a[466] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[467]
    var y = a[-468]
    a[467] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[468]
    var y = a[-469]
    a[468] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[469]
    var y = a[-470]
    a[469] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[470]
    var y = a[-471]
    a[470] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[471]
    var y = a[-472]
    a[471] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[472]
    var y = a[-473]
    a[472] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[473]
    var y = a[-474]
    a[473] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[474]
    var y = a[-475]
    a[474] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[475]
    var y = a[-476]
    a[475] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[476]
    var y = a[-477]
    a[476] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[477]
    var y = a[-478]
    a[477] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[478]
    var y = a[-479]
    a[478] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[479]
    var y = a[-480]
    a[479] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[480]
    var y = a[-481]
    a[480] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[481]
    var y = a[-482]
    a[481] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[482]
    var y = a[-483]
    a[482] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[483]
    var y = a[-484]
    a[483] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[484]
    var y = a[-485]
    a[484] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[485]
    var y = a[-486]
    a[485] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[486]
    var y = a[-487]
    a[486] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[487]
    var y = a[-488]
    a[487] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[488]
    var y = a[-489]
    a[488] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[489]
    var y = a[-490]
    a[489] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[490]
    var y = a[-491]
    a[490] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[491]
    var y = a[-492]
    a[491] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[492]
    var y = a[-493]
    a[492] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[493]
    var y = a[-494]
    a[493] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[494]
    var y = a[-495]
    a[494] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[495]
    var y = a[-496]
    a[495] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[496]
    var y = a[-497]
    a[496] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[497]
    var y = a[-498]
    a[497] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[498]
    var y = a[-499]
    a[498] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[499]
    var y = a[-500]
    a[499] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[500]
    var y = a[-501]
    a[500] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[501]
    var y = a[-502]
    a[501] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[502]
    var y = a[-503]
    a[502] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[503]
    var y = a[-504]
    a[503] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[504]
    var y = a[-505]
    a[504] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[505]
    var y = a[-506]
    a[505] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[506]
    var y = a[-507]
    a[506] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[507]
    var y = a[-508]
    a[507] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[508]
    var y = a[-509]
    a[508] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[509]
    var y = a[-510]
    a[509] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[510]
    var y = a[-511]
    a[510] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[511]
    var y = a[-512]
    a[511] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[512]
    var y = a[-513]
    a[512] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[513]
    var y = a[-514]
    a[513] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[514]
    var y = a[-515]
    a[514] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[515]
    var y = a[-516]
    a[515] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[516]
    var y = a[-517]
    a[516] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[517]
    var y = a[-518]
    a[517] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[518]
    var y = a[-519]
    a[518] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[519]
    var y = a[-520]
    a[519] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[520]
    var y = a[-521]
    a[520] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[521]
    var y = a[-522]
    a[521] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[522]
    var y = a[-523]
    a[522] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[523]
    var y = a[-524]
    a[523] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[524]
    var y = a[-525]
    a[524] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[525]
    var y = a[-526]
    a[525] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[526]
    var y = a[-527]
    a[526] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[527]
    var y = a[-528]
    a[527] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[528]
    var y = a[-529]
    a[528] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[529]
    var y = a[-530]
    a[529] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[530]
    var y = a[-531]
    a[530] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[531]
    var y = a[-532]
    a[531] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[532]
    var y = a[-533]
    a[532] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[533]
    var y = a[-534]
    a[533] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[534]
    var y = a[-535]
    a[534] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[535]
    var y = a[-536]
    a[535] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[536]
    var y = a[-537]
    a[536] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[537]
    var y = a[-538]
    a[537] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[538]
    var y = a[-539]
    a[538] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[539]
    var y = a[-540]
    a[539] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[540]
    var y = a[-541]
    a[540] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[541]
    var y = a[-542]
    a[541] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[542]
    var y = a[-543]
    a[542] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[543]
    var y = a[-544]
    a[543] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[544]
    var y = a[-545]
    a[544] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[545]
    var y = a[-546]
    a[545] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[546]
    var y = a[-547]
    a[546] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[547]
    var y = a[-548]
    a[547] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[548]
    var y = a[-549]
    a[548] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[549]
    var y = a[-550]
    a[549] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[550]
    var y = a[-551]
    a[550] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[551]
    var y = a[-552]
    a[551] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[552]
    var y = a[-553]
    a[552] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[553]
    var y = a[-554]
    a[553] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[554]
    var y = a[-555]
    a[554] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[555]
    var y = a[-556]
    a[555] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[556]
    var y = a[-557]
    a[556] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[557]
    var y = a[-558]
    a[557] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[558]
    var y = a[-559]
    a[558] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[559]
    var y = a[-560]
    a[559] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[560]
    var y = a[-561]
    a[560] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[561]
    var y = a[-562]
    a[561] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[562]
    var y = a[-563]
    a[562] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[563]
    var y = a[-564]
    a[563] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[564]
    var y = a[-565]
    a[564] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[565]
    var y = a[-566]
    a[565] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[566]
    var y = a[-567]
    a[566] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[567]
    var y = a[-568]
    a[567] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[568]
    var y = a[-569]
    a[568] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[569]
    var y = a[-570]
    a[569] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[570]
    var y = a[-571]
    a[570] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[571]
    var y = a[-572]
    a[571] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[572]
    var y = a[-573]
    a[572] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[573]
    var y = a[-574]
    a[573] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[574]
    var y = a[-575]
    a[574] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[575]
    var y = a[-576]
    a[575] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[576]
    var y = a[-577]
    a[576] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[577]
    var y = a[-578]
    a[577] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[578]
    var y = a[-579]
    a[578] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[579]
    var y = a[-580]
    a[579] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[580]
    var y = a[-581]
    a[580] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[581]
    var y = a[-582]
    a[581] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[582]
    var y = a[-583]
    a[582] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[583]
    var y = a[-584]
    a[583] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[584]
    var y = a[-585]
    a[584] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[585]
    var y = a[-586]
    a[585] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[586]
    var y = a[-587]
    a[586] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[587]
    var y = a[-588]
    a[587] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[588]
    var y = a[-589]
    a[588] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[589]
    var y = a[-590]
    a[589] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[590]
    var y = a[-591]
    a[590] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[591]
    var y = a[-592]
    a[591] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[592]
    var y = a[-593]
    a[592] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[593]
    var y = a[-594]
    a[593] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[594]
    var y = a[-595]
    a[594] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[595]
    var y = a[-596]
    a[595] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[596]
    var y = a[-597]
    a[596] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[597]
    var y = a[-598]
    a[597] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[598]
    var y = a[-599]
    a[598] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[599]
    var y = a[-600]
    a[599] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[600]
    var y = a[-601]
    a[600] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[601]
    var y = a[-602]
    a[601] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[602]
    var y = a[-603]
    a[602] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[603]
    var y = a[-604]
    a[603] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[604]
    var y = a[-605]
    a[604] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[605]
    var y = a[-606]
    a[605] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[606]
    var y = a[-607]
    a[606] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[607]
    var y = a[-608]
    a[607] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[608]
    var y = a[-609]
    a[608] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[609]
    var y = a[-610]
    a[609] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[610]
    var y = a[-611]
    a[610] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[611]
    var y = a[-612]
    a[611] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[612]
    var y = a[-613]
    a[612] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[613]
    var y = a[-614]
    a[613] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[614]
    var y = a[-615]
    a[614] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[615]
    var y = a[-616]
    a[615] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[616]
    var y = a[-617]
    a[616] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[617]
    var y = a[-618]
    a[617] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[618]
    var y = a[-619]
    a[618] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[619]
    var y = a[-620]
    a[619] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[620]
    var y = a[-621]
    a[620] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[621]
    var y = a[-622]
    a[621] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[622]
    var y = a[-623]
    a[622] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[623]
    var y = a[-624]
    a[623] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[624]
    var y = a[-625]
    a[624] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[625]
    var y = a[-626]
    a[625] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[626]
    var y = a[-627]
    a[626] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[627]
    var y = a[-628]
    a[627] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[628]
    var y = a[-629]
    a[628] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[629]
    var y = a[-630]
    a[629] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[630]
    var y = a[-631]
    a[630] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[631]
    var y = a[-632]
    a[631] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[632]
    var y = a[-633]
    a[632] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[633]
    var y = a[-634]
    a[633] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[634]
    var y = a[-635]
    a[634] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[635]
    var y = a[-636]
    a[635] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[636]
    var y = a[-637]
    a[636] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[637]
    var y = a[-638]
    a[637] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[638]
    var y = a[-639]
    a[638] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[639]
    var y = a[-640]
    a[639] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[640]
    var y = a[-641]
    a[640] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[641]
    var y = a[-642]
    a[641] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[642]
    var y = a[-643]
    a[642] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[643]
    var y = a[-644]
    a[643] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[644]
    var y = a[-645]
    a[644] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[645]
    var y = a[-646]
    a[645] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[646]
    var y = a[-647]
    a[646] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[647]
    var y = a[-648]
    a[647] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[648]
    var y = a[-649]
    a[648] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[649]
    var y = a[-650]
    a[649] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[650]
    var y = a[-651]
    a[650] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[651]
    var y = a[-652]
    a[651] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[652]
    var y = a[-653]
    a[652] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[653]
    var y = a[-654]
    a[653] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[654]
    var y = a[-655]
    a[654] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[655]
    var y = a[-656]
    a[655] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[656]
    var y = a[-657]
    a[656] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[657]
    var y = a[-658]
    a[657] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[658]
    var y = a[-659]
    a[658] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[659]
    var y = a[-660]
    a[659] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[660]
    var y = a[-661]
    a[660] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[661]
    var y = a[-662]
    a[661] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[662]
    var y = a[-663]
    a[662] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[663]
    var y = a[-664]
    a[663] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[664]
    var y = a[-665]
    a[664] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[665]
    var y = a[-666]
    a[665] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[666]
    var y = a[-667]
    a[666] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[667]
    var y = a[-668]
    a[667] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[668]
    var y = a[-669]
    a[668] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[669]
    var y = a[-670]
    a[669] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[670]
    var y = a[-671]
    a[670] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[671]
    var y = a[-672]
    a[671] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[672]
    var y = a[-673]
    a[672] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[673]
    var y = a[-674]
    a[673] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[674]
    var y = a[-675]
    a[674] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[675]
    var y = a[-676]
    a[675] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[676]
    var y = a[-677]
    a[676] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[677]
    var y = a[-678]
    a[677] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[678]
    var y = a[-679]
    a[678] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[679]
    var y = a[-680]
    a[679] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[680]
    var y = a[-681]
    a[680] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[681]
    var y = a[-682]
    a[681] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[682]
    var y = a[-683]
    a[682] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[683]
    var y = a[-684]
    a[683] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[684]
    var y = a[-685]
    a[684] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[685]
    var y = a[-686]
    a[685] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[686]
    var y = a[-687]
    a[686] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[687]
    var y = a[-688]
    a[687] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[688]
    var y = a[-689]
    a[688] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[689]
    var y = a[-690]
    a[689] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[690]
    var y = a[-691]
    a[690] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[691]
    var y = a[-692]
    a[691] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[692]
    var y = a[-693]
    a[692] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[693]
    var y = a[-694]
    a[693] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[694]
    var y = a[-695]
    a[694] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[695]
    var y = a[-696]
    a[695] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[696]
    var y = a[-697]
    a[696] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[697]
    var y = a[-698]
    a[697] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[698]
    var y = a[-699]
    a[698] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[699]
    var y = a[-700]
    a[699] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[700]
    var y = a[-701]
    a[700] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[701]
    var y = a[-702]
    a[701] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[702]
    var y = a[-703]
    a[702] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[703]
    var y = a[-704]
    a[703] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[704]
    var y = a[-705]
    a[704] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[705]
    var y = a[-706]
    a[705] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[706]
    var y = a[-707]
    a[706] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[707]
    var y = a[-708]
    a[707] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[708]
    var y = a[-709]
    a[708] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[709]
    var y = a[-710]
    a[709] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[710]
    var y = a[-711]
    a[710] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[711]
    var y = a[-712]
    a[711] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[712]
    var y = a[-713]
    a[712] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[713]
    var y = a[-714]
    a[713] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[714]
    var y = a[-715]
    a[714] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[715]
    var y = a[-716]
    a[715] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[716]
    var y = a[-717]
    a[716] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[717]
    var y = a[-718]
    a[717] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[718]
    var y = a[-719]
    a[718] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[719]
    var y = a[-720]
    a[719] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[720]
    var y = a[-721]
    a[720] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[721]
    var y = a[-722]
    a[721] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[722]
    var y = a[-723]
    a[722] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[723]
    var y = a[-724]
    a[723] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[724]
    var y = a[-725]
    a[724] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[725]
    var y = a[-726]
    a[725] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[726]
    var y = a[-727]
    a[726] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[727]
    var y = a[-728]
    a[727] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[728]
    var y = a[-729]
    a[728] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[729]
    var y = a[-730]
    a[729] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[730]
    var y = a[-731]
    a[730] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[731]
    var y = a[-732]
    a[731] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[732]
    var y = a[-733]
    a[732] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[733]
    var y = a[-734]
    a[733] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[734]
    var y = a[-735]
    a[734] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[735]
    var y = a[-736]
    a[735] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[736]
    var y = a[-737]
    a[736] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[737]
    var y = a[-738]
    a[737] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[738]
    var y = a[-739]
    a[738] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[739]
    var y = a[-740]
    a[739] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[740]
    var y = a[-741]
    a[740] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[741]
    var y = a[-742]
    a[741] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[742]
    var y = a[-743]
    a[742] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[743]
    var y = a[-744]
    a[743] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[744]
    var y = a[-745]
    a[744] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[745]
    var y = a[-746]
    a[745] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[746]
    var y = a[-747]
    a[746] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[747]
    var y = a[-748]
    a[747] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[748]
    var y = a[-749]
    a[748] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[749]
    var y = a[-750]
    a[749] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[750]
    var y = a[-751]
    a[750] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[751]
    var y = a[-752]
    a[751] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[752]
    var y = a[-753]
    a[752] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[753]
    var y = a[-754]
    a[753] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[754]
    var y = a[-755]
    a[754] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[755]
    var y = a[-756]
    a[755] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[756]
    var y = a[-757]
    a[756] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[757]
    var y = a[-758]
    a[757] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[758]
    var y = a[-759]
    a[758] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[759]
    var y = a[-760]
    a[759] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[760]
    var y = a[-761]
    a[760] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[761]
    var y = a[-762]
    a[761] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[762]
    var y = a[-763]
    a[762] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[763]
    var y = a[-764]
    a[763] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[764]
    var y = a[-765]
    a[764] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[765]
    var y = a[-766]
    a[765] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[766]
    var y = a[-767]
    a[766] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[767]
    var y = a[-768]
    a[767] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[768]
    var y = a[-769]
    a[768] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[769]
    var y = a[-770]
    a[769] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[770]
    var y = a[-771]
    a[770] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[771]
    var y = a[-772]
    a[771] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[772]
    var y = a[-773]
    a[772] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[773]
    var y = a[-774]
    a[773] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[774]
    var y = a[-775]
    a[774] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[775]
    var y = a[-776]
    a[775] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[776]
    var y = a[-777]
    a[776] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[777]
    var y = a[-778]
    a[777] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[778]
    var y = a[-779]
    a[778] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[779]
    var y = a[-780]
    a[779] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[780]
    var y = a[-781]
    a[780] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[781]
    var y = a[-782]
    a[781] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[782]
    var y = a[-783]
    a[782] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[783]
    var y = a[-784]
    a[783] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[784]
    var y = a[-785]
    a[784] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[785]
    var y = a[-786]
    a[785] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[786]
    var y = a[-787]
    a[786] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[787]
    var y = a[-788]
    a[787] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[788]
    var y = a[-789]
    a[788] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[789]
    var y = a[-790]
    a[789] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[790]
    var y = a[-791]
    a[790] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[791]
    var y = a[-792]
    a[791] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[792]
    var y = a[-793]
    a[792] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[793]
    var y = a[-794]
    a[793] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[794]
    var y = a[-795]
    a[794] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[795]
    var y = a[-796]
    a[795] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[796]
    var y = a[-797]
    a[796] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[797]
    var y = a[-798]
    a[797] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[798]
    var y = a[-799]
    a[798] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[799]
    var y = a[-800]
    a[799] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[800]
    var y = a[-801]
    a[800] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[801]
    var y = a[-802]
    a[801] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[802]
    var y = a[-803]
    a[802] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[803]
    var y = a[-804]
    a[803] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[804]
    var y = a[-805]
    a[804] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[805]
    var y = a[-806]
    a[805] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[806]
    var y = a[-807]
    a[806] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[807]
    var y = a[-808]
    a[807] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[808]
    var y = a[-809]
    a[808] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[809]
    var y = a[-810]
    a[809] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[810]
    var y = a[-811]
    a[810] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[811]
    var y = a[-812]
    a[811] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[812]
    var y = a[-813]
    a[812] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[813]
    var y = a[-814]
    a[813] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[814]
    var y = a[-815]
    a[814] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[815]
    var y = a[-816]
    a[815] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[816]
    var y = a[-817]
    a[816] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[817]
    var y = a[-818]
    a[817] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[818]
    var y = a[-819]
    a[818] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[819]
    var y = a[-820]
    a[819] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[820]
    var y = a[-821]
    a[820] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[821]
    var y = a[-822]
    a[821] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[822]
    var y = a[-823]
    a[822] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[823]
    var y = a[-824]
    a[823] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[824]
    var y = a[-825]
    a[824] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[825]
    var y = a[-826]
    a[825] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[826]
    var y = a[-827]
    a[826] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[827]
    var y = a[-828]
    a[827] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[828]
    var y = a[-829]
    a[828] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[829]
    var y = a[-830]
    a[829] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[830]
    var y = a[-831]
    a[830] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[831]
    var y = a[-832]
    a[831] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[832]
    var y = a[-833]
    a[832] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[833]
    var y = a[-834]
    a[833] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[834]
    var y = a[-835]
    a[834] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[835]
    var y = a[-836]
    a[835] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[836]
    var y = a[-837]
    a[836] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[837]
    var y = a[-838]
    a[837] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[838]
    var y = a[-839]
    a[838] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[839]
    var y = a[-840]
    a[839] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[840]
    var y = a[-841]
    a[840] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[841]
    var y = a[-842]
    a[841] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[842]
    var y = a[-843]
    a[842] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[843]
    var y = a[-844]
    a[843] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[844]
    var y = a[-845]
    a[844] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[845]
    var y = a[-846]
    a[845] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[846]
    var y = a[-847]
    a[846] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[847]
    var y = a[-848]
    a[847] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[848]
    var y = a[-849]
    a[848] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[849]
    var y = a[-850]
    a[849] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[850]
    var y = a[-851]
    a[850] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[851]
    var y = a[-852]
    a[851] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[852]
    var y = a[-853]
    a[852] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[853]
    var y = a[-854]
    a[853] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[854]
    var y = a[-855]
    a[854] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[855]
    var y = a[-856]
    a[855] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[856]
    var y = a[-857]
    a[856] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[857]
    var y = a[-858]
    a[857] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[858]
    var y = a[-859]
    a[858] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[859]
    var y = a[-860]
    a[859] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[860]
    var y = a[-861]
    a[860] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[861]
    var y = a[-862]
    a[861] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[862]
    var y = a[-863]
    a[862] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[863]
    var y = a[-864]
    a[863] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[864]
    var y = a[-865]
    a[864] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[865]
    var y = a[-866]
    a[865] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[866]
    var y = a[-867]
    a[866] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[867]
    var y = a[-868]
    a[867] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[868]
    var y = a[-869]
    a[868] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[869]
    var y = a[-870]
    a[869] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[870]
    var y = a[-871]
    a[870] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[871]
    var y = a[-872]
    a[871] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[872]
    var y = a[-873]
    a[872] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[873]
    var y = a[-874]
    a[873] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[874]
    var y = a[-875]
    a[874] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[875]
    var y = a[-876]
    a[875] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[876]
    var y = a[-877]
    a[876] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[877]
    var y = a[-878]
    a[877] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[878]
    var y = a[-879]
    a[878] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[879]
    var y = a[-880]
    a[879] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[880]
    var y = a[-881]
    a[880] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[881]
    var y = a[-882]
    a[881] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[882]
    var y = a[-883]
    a[882] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[883]
    var y = a[-884]
    a[883] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[884]
    var y = a[-885]
    a[884] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[885]
    var y = a[-886]
    a[885] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[886]
    var y = a[-887]
    a[886] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[887]
    var y = a[-888]
    a[887] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[888]
    var y = a[-889]
    a[888] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[889]
    var y = a[-890]
    a[889] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[890]
    var y = a[-891]
    a[890] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[891]
    var y = a[-892]
    a[891] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[892]
    var y = a[-893]
    a[892] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[893]
    var y = a[-894]
    a[893] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[894]
    var y = a[-895]
    a[894] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[895]
    var y = a[-896]
    a[895] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[896]
    var y = a[-897]
    a[896] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[897]
    var y = a[-898]
    a[897] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[898]
    var y = a[-899]
    a[898] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[899]
    var y = a[-900]
    a[899] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[900]
    var y = a[-901]
    a[900] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[901]
    var y = a[-902]
    a[901] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[902]
    var y = a[-903]
    a[902] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[903]
    var y = a[-904]
    a[903] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[904]
    var y = a[-905]
    a[904] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[905]
    var y = a[-906]
    a[905] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[906]
    var y = a[-907]
    a[906] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[907]
    var y = a[-908]
    a[907] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[908]
    var y = a[-909]
    a[908] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[909]
    var y = a[-910]
    a[909] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[910]
    var y = a[-911]
    a[910] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[911]
    var y = a[-912]
    a[911] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[912]
    var y = a[-913]
    a[912] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[913]
    var y = a[-914]
    a[913] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[914]
    var y = a[-915]
    a[914] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[915]
    var y = a[-916]
    a[915] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[916]
    var y = a[-917]
    a[916] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[917]
    var y = a[-918]
    a[917] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[918]
    var y = a[-919]
    a[918] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[919]
    var y = a[-920]
    a[919] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[920]
    var y = a[-921]
    a[920] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[921]
    var y = a[-922]
    a[921] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[922]
    var y = a[-923]
    a[922] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[923]
    var y = a[-924]
    a[923] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[924]
    var y = a[-925]
    a[924] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[925]
    var y = a[-926]
    a[925] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[926]
    var y = a[-927]
    a[926] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[927]
    var y = a[-928]
    a[927] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[928]
    var y = a[-929]
    a[928] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[929]
    var y = a[-930]
    a[929] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[930]
    var y = a[-931]
    a[930] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[931]
    var y = a[-932]
    a[931] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[932]
    var y = a[-933]
    a[932] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[933]
    var y = a[-934]
    a[933] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[934]
    var y = a[-935]
    a[934] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[935]
    var y = a[-936]
    a[935] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[936]
    var y = a[-937]
    a[936] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[937]
    var y = a[-938]
    a[937] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[938]
    var y = a[-939]
    a[938] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[939]
    var y = a[-940]
    a[939] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[940]
    var y = a[-941]
    a[940] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[941]
    var y = a[-942]
    a[941] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[942]
    var y = a[-943]
    a[942] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[943]
    var y = a[-944]
    a[943] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[944]
    var y = a[-945]
    a[944] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[945]
    var y = a[-946]
    a[945] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[946]
    var y = a[-947]
    a[946] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[947]
    var y = a[-948]
    a[947] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[948]
    var y = a[-949]
    a[948] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[949]
    var y = a[-950]
    a[949] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[950]
    var y = a[-951]
    a[950] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[951]
    var y = a[-952]
    a[951] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[952]
    var y = a[-953]
    a[952] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[953]
    var y = a[-954]
    a[953] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[954]
    var y = a[-955]
    a[954] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[955]
    var y = a[-956]
    a[955] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[956]
    var y = a[-957]
    a[956] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[957]
    var y = a[-958]
    a[957] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[958]
    var y = a[-959]
    a[958] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[959]
    var y = a[-960]
    a[959] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[960]
    var y = a[-961]
    a[960] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[961]
    var y = a[-962]
    a[961] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[962]
    var y = a[-963]
    a[962] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[963]
    var y = a[-964]
    a[963] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[964]
    var y = a[-965]
    a[964] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[965]
    var y = a[-966]
    a[965] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[966]
    var y = a[-967]
    a[966] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[967]
    var y = a[-968]
    a[967] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[968]
    var y = a[-969]
    a[968] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[969]
    var y = a[-970]
    a[969] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[970]
    var y = a[-971]
    a[970] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[971]
    var y = a[-972]
    a[971] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[972]
    var y = a[-973]
    a[972] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[973]
    var y = a[-974]
    a[973] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[974]
    var y = a[-975]
    a[974] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[975]
    var y = a[-976]
    a[975] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[976]
    var y = a[-977]
    a[976] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[977]
    var y = a[-978]
    a[977] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[978]
    var y = a[-979]
    a[978] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[979]
    var y = a[-980]
    a[979] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[980]
    var y = a[-981]
    a[980] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[981]
    var y = a[-982]
    a[981] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[982]
    var y = a[-983]
    a[982] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[983]
    var y = a[-984]
    a[983] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[984]
    var y = a[-985]
    a[984] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[985]
    var y = a[-986]
    a[985] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[986]
    var y = a[-987]
    a[986] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[987]
    var y = a[-988]
    a[987] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[988]
    var y = a[-989]
    a[988] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[989]
    var y = a[-990]
    a[989] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[990]
    var y = a[-991]
    a[990] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[991]
    var y = a[-992]
    a[991] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[992]
    var y = a[-993]
    a[992] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[993]
    var y = a[-994]
    a[993] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[994]
    var y = a[-995]
    a[994] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[995]
    var y = a[-996]
    a[995] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[996]
    var y = a[-997]
    a[996] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[997]
    var y = a[-998]
    a[997] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[998]
    var y = a[-999]
    a[998] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[999]
    var y = a[-1000]
    a[999] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1000]
    var y = a[-1001]
    a[1000] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1001]
    var y = a[-1002]
    a[1001] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1002]
    var y = a[-1003]
    a[1002] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1003]
    var y = a[-1004]
    a[1003] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1004]
    var y = a[-1005]
    a[1004] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1005]
    var y = a[-1006]
    a[1005] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1006]
    var y = a[-1007]
    a[1006] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1007]
    var y = a[-1008]
    a[1007] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1008]
    var y = a[-1009]
    a[1008] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1009]
    var y = a[-1010]
    a[1009] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1010]
    var y = a[-1011]
    a[1010] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1011]
    var y = a[-1012]
    a[1011] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1012]
    var y = a[-1013]
    a[1012] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1013]
    var y = a[-1014]
    a[1013] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1014]
    var y = a[-1015]
    a[1014] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1015]
    var y = a[-1016]
    a[1015] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1016]
    var y = a[-1017]
    a[1016] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1017]
    var y = a[-1018]
    a[1017] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1018]
    var y = a[-1019]
    a[1018] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1019]
    var y = a[-1020]
    a[1019] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1020]
    var y = a[-1021]
    a[1020] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1021]
    var y = a[-1022]
    a[1021] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1022]
    var y = a[-1023]
    a[1022] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1023]
    var y = a[-1024]
    a[1023] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1024]
    var y = a[-1025]
    a[1024] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1025]
    var y = a[-1026]
    a[1025] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1026]
    var y = a[-1027]
    a[1026] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1027]
    var y = a[-1028]
    a[1027] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1028]
    var y = a[-1029]
    a[1028] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1029]
    var y = a[-1030]
    a[1029] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1030]
    var y = a[-1031]
    a[1030] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1031]
    var y = a[-1032]
    a[1031] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1032]
    var y = a[-1033]
    a[1032] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1033]
    var y = a[-1034]
    a[1033] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1034]
    var y = a[-1035]
    a[1034] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1035]
    var y = a[-1036]
    a[1035] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1036]
    var y = a[-1037]
    a[1036] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1037]
    var y = a[-1038]
    a[1037] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1038]
    var y = a[-1039]
    a[1038] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1039]
    var y = a[-1040]
    a[1039] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1040]
    var y = a[-1041]
    a[1040] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1041]
    var y = a[-1042]
    a[1041] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1042]
    var y = a[-1043]
    a[1042] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1043]
    var y = a[-1044]
    a[1043] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1044]
    var y = a[-1045]
    a[1044] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1045]
    var y = a[-1046]
    a[1045] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1046]
    var y = a[-1047]
    a[1046] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1047]
    var y = a[-1048]
    a[1047] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1048]
    var y = a[-1049]
    a[1048] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1049]
    var y = a[-1050]
    a[1049] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1050]
    var y = a[-1051]
    a[1050] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1051]
    var y = a[-1052]
    a[1051] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1052]
    var y = a[-1053]
    a[1052] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1053]
    var y = a[-1054]
    a[1053] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1054]
    var y = a[-1055]
    a[1054] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1055]
    var y = a[-1056]
    a[1055] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1056]
    var y = a[-1057]
    a[1056] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1057]
    var y = a[-1058]
    a[1057] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1058]
    var y = a[-1059]
    a[1058] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1059]
    var y = a[-1060]
    a[1059] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1060]
    var y = a[-1061]
    a[1060] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1061]
    var y = a[-1062]
    a[1061] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1062]
    var y = a[-1063]
    a[1062] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1063]
    var y = a[-1064]
    a[1063] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1064]
    var y = a[-1065]
    a[1064] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1065]
    var y = a[-1066]
    a[1065] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1066]
    var y = a[-1067]
    a[1066] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1067]
    var y = a[-1068]
    a[1067] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1068]
    var y = a[-1069]
    a[1068] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1069]
    var y = a[-1070]
    a[1069] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1070]
    var y = a[-1071]
    a[1070] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1071]
    var y = a[-1072]
    a[1071] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1072]
    var y = a[-1073]
    a[1072] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1073]
    var y = a[-1074]
    a[1073] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1074]
    var y = a[-1075]
    a[1074] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1075]
    var y = a[-1076]
    a[1075] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1076]
    var y = a[-1077]
    a[1076] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1077]
    var y = a[-1078]
    a[1077] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1078]
    var y = a[-1079]
    a[1078] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1079]
    var y = a[-1080]
    a[1079] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1080]
    var y = a[-1081]
    a[1080] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1081]
    var y = a[-1082]
    a[1081] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1082]
    var y = a[-1083]
    a[1082] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1083]
    var y = a[-1084]
    a[1083] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1084]
    var y = a[-1085]
    a[1084] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1085]
    var y = a[-1086]
    a[1085] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1086]
    var y = a[-1087]
    a[1086] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1087]
    var y = a[-1088]
    a[1087] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1088]
    var y = a[-1089]
    a[1088] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1089]
    var y = a[-1090]
    a[1089] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1090]
    var y = a[-1091]
    a[1090] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1091]
    var y = a[-1092]
    a[1091] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1092]
    var y = a[-1093]
    a[1092] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1093]
    var y = a[-1094]
    a[1093] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1094]
    var y = a[-1095]
    a[1094] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1095]
    var y = a[-1096]
    a[1095] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1096]
    var y = a[-1097]
    a[1096] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1097]
    var y = a[-1098]
    a[1097] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1098]
    var y = a[-1099]
    a[1098] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1099]
    var y = a[-1100]
    a[1099] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1100]
    var y = a[-1101]
    a[1100] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1101]
    var y = a[-1102]
    a[1101] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1102]
    var y = a[-1103]
    a[1102] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1103]
    var y = a[-1104]
    a[1103] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1104]
    var y = a[-1105]
    a[1104] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1105]
    var y = a[-1106]
    a[1105] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1106]
    var y = a[-1107]
    a[1106] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1107]
    var y = a[-1108]
    a[1107] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1108]
    var y = a[-1109]
    a[1108] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1109]
    var y = a[-1110]
    a[1109] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1110]
    var y = a[-1111]
    a[1110] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1111]
    var y = a[-1112]
    a[1111] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1112]
    var y = a[-1113]
    a[1112] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1113]
    var y = a[-1114]
    a[1113] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1114]
    var y = a[-1115]
    a[1114] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1115]
    var y = a[-1116]
    a[1115] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1116]
    var y = a[-1117]
    a[1116] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1117]
    var y = a[-1118]
    a[1117] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1118]
    var y = a[-1119]
    a[1118] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1119]
    var y = a[-1120]
    a[1119] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1120]
    var y = a[-1121]
    a[1120] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1121]
    var y = a[-1122]
    a[1121] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1122]
    var y = a[-1123]
    a[1122] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1123]
    var y = a[-1124]
    a[1123] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1124]
    var y = a[-1125]
    a[1124] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1125]
    var y = a[-1126]
    a[1125] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1126]
    var y = a[-1127]
    a[1126] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1127]
    var y = a[-1128]
    a[1127] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1128]
    var y = a[-1129]
    a[1128] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1129]
    var y = a[-1130]
    a[1129] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1130]
    var y = a[-1131]
    a[1130] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1131]
    var y = a[-1132]
    a[1131] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1132]
    var y = a[-1133]
    a[1132] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1133]
    var y = a[-1134]
    a[1133] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1134]
    var y = a[-1135]
    a[1134] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1135]
    var y = a[-1136]
    a[1135] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1136]
    var y = a[-1137]
    a[1136] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1137]
    var y = a[-1138]
    a[1137] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1138]
    var y = a[-1139]
    a[1138] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1139]
    var y = a[-1140]
    a[1139] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1140]
    var y = a[-1141]
    a[1140] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1141]
    var y = a[-1142]
    a[1141] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1142]
    var y = a[-1143]
    a[1142] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1143]
    var y = a[-1144]
    a[1143] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1144]
    var y = a[-1145]
    a[1144] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1145]
    var y = a[-1146]
    a[1145] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1146]
    var y = a[-1147]
    a[1146] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1147]
    var y = a[-1148]
    a[1147] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1148]
    var y = a[-1149]
    a[1148] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1149]
    var y = a[-1150]
    a[1149] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1150]
    var y = a[-1151]
    a[1150] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1151]
    var y = a[-1152]
    a[1151] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1152]
    var y = a[-1153]
    a[1152] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1153]
    var y = a[-1154]
    a[1153] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1154]
    var y = a[-1155]
    a[1154] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1155]
    var y = a[-1156]
    a[1155] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1156]
    var y = a[-1157]
    a[1156] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1157]
    var y = a[-1158]
    a[1157] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1158]
    var y = a[-1159]
    a[1158] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1159]
    var y = a[-1160]
    a[1159] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1160]
    var y = a[-1161]
    a[1160] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1161]
    var y = a[-1162]
    a[1161] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1162]
    var y = a[-1163]
    a[1162] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1163]
    var y = a[-1164]
    a[1163] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1164]
    var y = a[-1165]
    a[1164] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1165]
    var y = a[-1166]
    a[1165] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1166]
    var y = a[-1167]
    a[1166] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1167]
    var y = a[-1168]
    a[1167] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1168]
    var y = a[-1169]
    a[1168] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1169]
    var y = a[-1170]
    a[1169] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1170]
    var y = a[-1171]
    a[1170] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1171]
    var y = a[-1172]
    a[1171] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1172]
    var y = a[-1173]
    a[1172] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1173]
    var y = a[-1174]
    a[1173] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1174]
    var y = a[-1175]
    a[1174] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1175]
    var y = a[-1176]
    a[1175] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1176]
    var y = a[-1177]
    a[1176] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1177]
    var y = a[-1178]
    a[1177] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1178]
    var y = a[-1179]
    a[1178] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1179]
    var y = a[-1180]
    a[1179] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1180]
    var y = a[-1181]
    a[1180] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1181]
    var y = a[-1182]
    a[1181] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1182]
    var y = a[-1183]
    a[1182] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1183]
    var y = a[-1184]
    a[1183] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1184]
    var y = a[-1185]
    a[1184] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1185]
    var y = a[-1186]
    a[1185] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1186]
    var y = a[-1187]
    a[1186] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1187]
    var y = a[-1188]
    a[1187] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1188]
    var y = a[-1189]
    a[1188] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1189]
    var y = a[-1190]
    a[1189] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1190]
    var y = a[-1191]
    a[1190] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1191]
    var y = a[-1192]
    a[1191] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1192]
    var y = a[-1193]
    a[1192] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1193]
    var y = a[-1194]
    a[1193] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1194]
    var y = a[-1195]
    a[1194] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1195]
    var y = a[-1196]
    a[1195] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1196]
    var y = a[-1197]
    a[1196] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1197]
    var y = a[-1198]
    a[1197] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1198]
    var y = a[-1199]
    a[1198] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1199]
    var y = a[-1200]
    a[1199] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1200]
    var y = a[-1201]
    a[1200] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1201]
    var y = a[-1202]
    a[1201] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1202]
    var y = a[-1203]
    a[1202] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1203]
    var y = a[-1204]
    a[1203] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1204]
    var y = a[-1205]
    a[1204] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1205]
    var y = a[-1206]
    a[1205] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1206]
    var y = a[-1207]
    a[1206] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1207]
    var y = a[-1208]
    a[1207] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1208]
    var y = a[-1209]
    a[1208] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1209]
    var y = a[-1210]
    a[1209] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1210]
    var y = a[-1211]
    a[1210] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1211]
    var y = a[-1212]
    a[1211] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1212]
    var y = a[-1213]
    a[1212] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1213]
    var y = a[-1214]
    a[1213] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1214]
    var y = a[-1215]
    a[1214] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1215]
    var y = a[-1216]
    a[1215] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1216]
    var y = a[-1217]
    a[1216] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1217]
    var y = a[-1218]
    a[1217] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1218]
    var y = a[-1219]
    a[1218] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1219]
    var y = a[-1220]
    a[1219] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1220]
    var y = a[-1221]
    a[1220] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1221]
    var y = a[-1222]
    a[1221] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1222]
    var y = a[-1223]
    a[1222] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1223]
    var y = a[-1224]
    a[1223] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1224]
    var y = a[-1225]
    a[1224] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1225]
    var y = a[-1226]
    a[1225] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1226]
    var y = a[-1227]
    a[1226] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1227]
    var y = a[-1228]
    a[1227] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1228]
    var y = a[-1229]
    a[1228] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1229]
    var y = a[-1230]
    a[1229] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1230]
    var y = a[-1231]
    a[1230] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1231]
    var y = a[-1232]
    a[1231] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1232]
    var y = a[-1233]
    a[1232] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1233]
    var y = a[-1234]
    a[1233] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1234]
    var y = a[-1235]
    a[1234] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1235]
    var y = a[-1236]
    a[1235] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1236]
    var y = a[-1237]
    a[1236] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1237]
    var y = a[-1238]
    a[1237] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1238]
    var y = a[-1239]
    a[1238] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1239]
    var y = a[-1240]
    a[1239] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1240]
    var y = a[-1241]
    a[1240] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1241]
    var y = a[-1242]
    a[1241] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1242]
    var y = a[-1243]
    a[1242] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1243]
    var y = a[-1244]
    a[1243] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1244]
    var y = a[-1245]
    a[1244] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1245]
    var y = a[-1246]
    a[1245] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1246]
    var y = a[-1247]
    a[1246] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1247]
    var y = a[-1248]
    a[1247] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1248]
    var y = a[-1249]
    a[1248] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1249]
    var y = a[-1250]
    a[1249] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1250]
    var y = a[-1251]
    a[1250] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1251]
    var y = a[-1252]
    a[1251] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1252]
    var y = a[-1253]
    a[1252] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1253]
    var y = a[-1254]
    a[1253] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1254]
    var y = a[-1255]
    a[1254] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1255]
    var y = a[-1256]
    a[1255] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1256]
    var y = a[-1257]
    a[1256] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1257]
    var y = a[-1258]
    a[1257] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1258]
    var y = a[-1259]
    a[1258] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1259]
    var y = a[-1260]
    a[1259] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1260]
    var y = a[-1261]
    a[1260] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1261]
    var y = a[-1262]
    a[1261] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1262]
    var y = a[-1263]
    a[1262] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1263]
    var y = a[-1264]
    a[1263] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1264]
    var y = a[-1265]
    a[1264] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1265]
    var y = a[-1266]
    a[1265] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1266]
    var y = a[-1267]
    a[1266] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1267]
    var y = a[-1268]
    a[1267] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1268]
    var y = a[-1269]
    a[1268] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1269]
    var y = a[-1270]
    a[1269] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1270]
    var y = a[-1271]
    a[1270] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1271]
    var y = a[-1272]
    a[1271] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1272]
    var y = a[-1273]
    a[1272] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1273]
    var y = a[-1274]
    a[1273] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1274]
    var y = a[-1275]
    a[1274] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1275]
    var y = a[-1276]
    a[1275] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1276]
    var y = a[-1277]
    a[1276] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1277]
    var y = a[-1278]
    a[1277] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1278]
    var y = a[-1279]
    a[1278] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1279]
    var y = a[-1280]
    a[1279] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1280]
    var y = a[-1281]
    a[1280] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1281]
    var y = a[-1282]
    a[1281] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1282]
    var y = a[-1283]
    a[1282] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1283]
    var y = a[-1284]
    a[1283] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1284]
    var y = a[-1285]
    a[1284] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1285]
    var y = a[-1286]
    a[1285] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1286]
    var y = a[-1287]
    a[1286] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1287]
    var y = a[-1288]
    a[1287] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1288]
    var y = a[-1289]
    a[1288] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1289]
    var y = a[-1290]
    a[1289] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1290]
    var y = a[-1291]
    a[1290] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1291]
    var y = a[-1292]
    a[1291] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1292]
    var y = a[-1293]
    a[1292] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1293]
    var y = a[-1294]
    a[1293] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1294]
    var y = a[-1295]
    a[1294] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1295]
    var y = a[-1296]
    a[1295] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1296]
    var y = a[-1297]
    a[1296] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1297]
    var y = a[-1298]
    a[1297] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1298]
    var y = a[-1299]
    a[1298] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1299]
    var y = a[-1300]
    a[1299] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1300]
    var y = a[-1301]
    a[1300] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1301]
    var y = a[-1302]
    a[1301] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1302]
    var y = a[-1303]
    a[1302] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1303]
    var y = a[-1304]
    a[1303] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1304]
    var y = a[-1305]
    a[1304] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1305]
    var y = a[-1306]
    a[1305] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1306]
    var y = a[-1307]
    a[1306] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1307]
    var y = a[-1308]
    a[1307] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1308]
    var y = a[-1309]
    a[1308] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1309]
    var y = a[-1310]
    a[1309] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1310]
    var y = a[-1311]
    a[1310] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1311]
    var y = a[-1312]
    a[1311] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1312]
    var y = a[-1313]
    a[1312] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1313]
    var y = a[-1314]
    a[1313] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1314]
    var y = a[-1315]
    a[1314] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1315]
    var y = a[-1316]
    a[1315] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1316]
    var y = a[-1317]
    a[1316] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1317]
    var y = a[-1318]
    a[1317] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1318]
    var y = a[-1319]
    a[1318] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1319]
    var y = a[-1320]
    a[1319] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1320]
    var y = a[-1321]
    a[1320] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1321]
    var y = a[-1322]
    a[1321] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1322]
    var y = a[-1323]
    a[1322] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1323]
    var y = a[-1324]
    a[1323] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1324]
    var y = a[-1325]
    a[1324] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1325]
    var y = a[-1326]
    a[1325] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1326]
    var y = a[-1327]
    a[1326] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1327]
    var y = a[-1328]
    a[1327] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1328]
    var y = a[-1329]
    a[1328] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1329]
    var y = a[-1330]
    a[1329] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1330]
    var y = a[-1331]
    a[1330] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1331]
    var y = a[-1332]
    a[1331] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1332]
    var y = a[-1333]
    a[1332] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1333]
    var y = a[-1334]
    a[1333] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1334]
    var y = a[-1335]
    a[1334] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1335]
    var y = a[-1336]
    a[1335] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1336]
    var y = a[-1337]
    a[1336] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1337]
    var y = a[-1338]
    a[1337] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1338]
    var y = a[-1339]
    a[1338] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1339]
    var y = a[-1340]
    a[1339] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1340]
    var y = a[-1341]
    a[1340] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1341]
    var y = a[-1342]
    a[1341] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1342]
    var y = a[-1343]
    a[1342] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1343]
    var y = a[-1344]
    a[1343] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1344]
    var y = a[-1345]
    a[1344] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1345]
    var y = a[-1346]
    a[1345] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1346]
    var y = a[-1347]
    a[1346] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1347]
    var y = a[-1348]
    a[1347] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1348]
    var y = a[-1349]
    a[1348] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1349]
    var y = a[-1350]
    a[1349] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1350]
    var y = a[-1351]
    a[1350] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1351]
    var y = a[-1352]
    a[1351] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1352]
    var y = a[-1353]
    a[1352] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1353]
    var y = a[-1354]
    a[1353] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1354]
    var y = a[-1355]
    a[1354] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1355]
    var y = a[-1356]
    a[1355] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1356]
    var y = a[-1357]
    a[1356] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1357]
    var y = a[-1358]
    a[1357] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1358]
    var y = a[-1359]
    a[1358] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1359]
    var y = a[-1360]
    a[1359] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1360]
    var y = a[-1361]
    a[1360] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1361]
    var y = a[-1362]
    a[1361] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1362]
    var y = a[-1363]
    a[1362] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1363]
    var y = a[-1364]
    a[1363] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1364]
    var y = a[-1365]
    a[1364] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1365]
    var y = a[-1366]
    a[1365] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1366]
    var y = a[-1367]
    a[1366] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1367]
    var y = a[-1368]
    a[1367] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1368]
    var y = a[-1369]
    a[1368] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1369]
    var y = a[-1370]
    a[1369] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1370]
    var y = a[-1371]
    a[1370] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1371]
    var y = a[-1372]
    a[1371] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1372]
    var y = a[-1373]
    a[1372] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1373]
    var y = a[-1374]
    a[1373] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1374]
    var y = a[-1375]
    a[1374] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1375]
    var y = a[-1376]
    a[1375] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1376]
    var y = a[-1377]
    a[1376] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1377]
    var y = a[-1378]
    a[1377] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1378]
    var y = a[-1379]
    a[1378] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1379]
    var y = a[-1380]
    a[1379] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1380]
    var y = a[-1381]
    a[1380] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1381]
    var y = a[-1382]
    a[1381] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1382]
    var y = a[-1383]
    a[1382] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1383]
    var y = a[-1384]
    a[1383] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1384]
    var y = a[-1385]
    a[1384] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1385]
    var y = a[-1386]
    a[1385] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1386]
    var y = a[-1387]
    a[1386] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1387]
    var y = a[-1388]
    a[1387] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1388]
    var y = a[-1389]
    a[1388] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1389]
    var y = a[-1390]
    a[1389] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1390]
    var y = a[-1391]
    a[1390] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1391]
    var y = a[-1392]
    a[1391] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1392]
    var y = a[-1393]
    a[1392] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1393]
    var y = a[-1394]
    a[1393] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1394]
    var y = a[-1395]
    a[1394] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1395]
    var y = a[-1396]
    a[1395] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1396]
    var y = a[-1397]
    a[1396] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1397]
    var y = a[-1398]
    a[1397] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1398]
    var y = a[-1399]
    a[1398] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1399]
    var y = a[-1400]
    a[1399] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1400]
    var y = a[-1401]
    a[1400] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1401]
    var y = a[-1402]
    a[1401] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1402]
    var y = a[-1403]
    a[1402] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1403]
    var y = a[-1404]
    a[1403] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1404]
    var y = a[-1405]
    a[1404] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1405]
    var y = a[-1406]
    a[1405] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1406]
    var y = a[-1407]
    a[1406] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1407]
    var y = a[-1408]
    a[1407] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1408]
    var y = a[-1409]
    a[1408] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1409]
    var y = a[-1410]
    a[1409] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1410]
    var y = a[-1411]
    a[1410] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1411]
    var y = a[-1412]
    a[1411] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1412]
    var y = a[-1413]
    a[1412] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1413]
    var y = a[-1414]
    a[1413] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1414]
    var y = a[-1415]
    a[1414] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1415]
    var y = a[-1416]
    a[1415] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1416]
    var y = a[-1417]
    a[1416] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1417]
    var y = a[-1418]
    a[1417] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1418]
    var y = a[-1419]
    a[1418] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1419]
    var y = a[-1420]
    a[1419] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1420]
    var y = a[-1421]
    a[1420] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1421]
    var y = a[-1422]
    a[1421] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1422]
    var y = a[-1423]
    a[1422] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1423]
    var y = a[-1424]
    a[1423] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1424]
    var y = a[-1425]
    a[1424] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1425]
    var y = a[-1426]
    a[1425] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1426]
    var y = a[-1427]
    a[1426] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1427]
    var y = a[-1428]
    a[1427] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1428]
    var y = a[-1429]
    a[1428] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1429]
    var y = a[-1430]
    a[1429] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1430]
    var y = a[-1431]
    a[1430] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1431]
    var y = a[-1432]
    a[1431] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1432]
    var y = a[-1433]
    a[1432] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1433]
    var y = a[-1434]
    a[1433] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1434]
    var y = a[-1435]
    a[1434] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1435]
    var y = a[-1436]
    a[1435] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1436]
    var y = a[-1437]
    a[1436] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1437]
    var y = a[-1438]
    a[1437] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1438]
    var y = a[-1439]
    a[1438] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1439]
    var y = a[-1440]
    a[1439] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1440]
    var y = a[-1441]
    a[1440] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1441]
    var y = a[-1442]
    a[1441] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1442]
    var y = a[-1443]
    a[1442] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1443]
    var y = a[-1444]
    a[1443] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1444]
    var y = a[-1445]
    a[1444] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1445]
    var y = a[-1446]
    a[1445] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1446]
    var y = a[-1447]
    a[1446] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1447]
    var y = a[-1448]
    a[1447] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1448]
    var y = a[-1449]
    a[1448] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1449]
    var y = a[-1450]
    a[1449] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1450]
    var y = a[-1451]
    a[1450] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1451]
    var y = a[-1452]
    a[1451] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1452]
    var y = a[-1453]
    a[1452] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1453]
    var y = a[-1454]
    a[1453] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1454]
    var y = a[-1455]
    a[1454] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1455]
    var y = a[-1456]
    a[1455] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1456]
    var y = a[-1457]
    a[1456] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1457]
    var y = a[-1458]
    a[1457] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1458]
    var y = a[-1459]
    a[1458] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1459]
    var y = a[-1460]
    a[1459] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1460]
    var y = a[-1461]
    a[1460] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1461]
    var y = a[-1462]
    a[1461] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1462]
    var y = a[-1463]
    a[1462] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1463]
    var y = a[-1464]
    a[1463] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1464]
    var y = a[-1465]
    a[1464] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1465]
    var y = a[-1466]
    a[1465] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1466]
    var y = a[-1467]
    a[1466] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1467]
    var y = a[-1468]
    a[1467] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1468]
    var y = a[-1469]
    a[1468] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1469]
    var y = a[-1470]
    a[1469] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1470]
    var y = a[-1471]
    a[1470] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1471]
    var y = a[-1472]
    a[1471] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1472]
    var y = a[-1473]
    a[1472] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1473]
    var y = a[-1474]
    a[1473] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1474]
    var y = a[-1475]
    a[1474] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1475]
    var y = a[-1476]
    a[1475] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1476]
    var y = a[-1477]
    a[1476] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1477]
    var y = a[-1478]
    a[1477] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1478]
    var y = a[-1479]
    a[1478] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1479]
    var y = a[-1480]
    a[1479] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1480]
    var y = a[-1481]
    a[1480] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1481]
    var y = a[-1482]
    a[1481] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1482]
    var y = a[-1483]
    a[1482] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1483]
    var y = a[-1484]
    a[1483] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1484]
    var y = a[-1485]
    a[1484] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1485]
    var y = a[-1486]
    a[1485] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1486]
    var y = a[-1487]
    a[1486] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1487]
    var y = a[-1488]
    a[1487] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1488]
    var y = a[-1489]
    a[1488] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1489]
    var y = a[-1490]
    a[1489] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1490]
    var y = a[-1491]
    a[1490] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1491]
    var y = a[-1492]
    a[1491] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1492]
    var y = a[-1493]
    a[1492] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1493]
    var y = a[-1494]
    a[1493] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1494]
    var y = a[-1495]
    a[1494] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1495]
    var y = a[-1496]
    a[1495] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1496]
    var y = a[-1497]
    a[1496] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1497]
    var y = a[-1498]
    a[1497] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1498]
    var y = a[-1499]
    a[1498] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1499]
    var y = a[-1500]
    a[1499] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1500]
    var y = a[-1501]
    a[1500] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1501]
    var y = a[-1502]
    a[1501] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1502]
    var y = a[-1503]
    a[1502] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1503]
    var y = a[-1504]
    a[1503] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1504]
    var y = a[-1505]
    a[1504] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1505]
    var y = a[-1506]
    a[1505] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1506]
    var y = a[-1507]
    a[1506] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1507]
    var y = a[-1508]
    a[1507] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1508]
    var y = a[-1509]
    a[1508] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1509]
    var y = a[-1510]
    a[1509] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1510]
    var y = a[-1511]
    a[1510] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1511]
    var y = a[-1512]
    a[1511] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1512]
    var y = a[-1513]
    a[1512] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1513]
    var y = a[-1514]
    a[1513] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1514]
    var y = a[-1515]
    a[1514] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1515]
    var y = a[-1516]
    a[1515] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1516]
    var y = a[-1517]
    a[1516] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1517]
    var y = a[-1518]
    a[1517] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1518]
    var y = a[-1519]
    a[1518] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1519]
    var y = a[-1520]
    a[1519] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1520]
    var y = a[-1521]
    a[1520] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1521]
    var y = a[-1522]
    a[1521] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1522]
    var y = a[-1523]
    a[1522] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1523]
    var y = a[-1524]
    a[1523] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1524]
    var y = a[-1525]
    a[1524] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1525]
    var y = a[-1526]
    a[1525] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1526]
    var y = a[-1527]
    a[1526] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1527]
    var y = a[-1528]
    a[1527] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1528]
    var y = a[-1529]
    a[1528] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1529]
    var y = a[-1530]
    a[1529] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1530]
    var y = a[-1531]
    a[1530] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1531]
    var y = a[-1532]
    a[1531] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1532]
    var y = a[-1533]
    a[1532] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1533]
    var y = a[-1534]
    a[1533] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1534]
    var y = a[-1535]
    a[1534] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1535]
    var y = a[-1536]
    a[1535] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1536]
    var y = a[-1537]
    a[1536] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1537]
    var y = a[-1538]
    a[1537] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1538]
    var y = a[-1539]
    a[1538] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1539]
    var y = a[-1540]
    a[1539] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1540]
    var y = a[-1541]
    a[1540] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1541]
    var y = a[-1542]
    a[1541] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1542]
    var y = a[-1543]
    a[1542] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1543]
    var y = a[-1544]
    a[1543] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1544]
    var y = a[-1545]
    a[1544] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1545]
    var y = a[-1546]
    a[1545] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1546]
    var y = a[-1547]
    a[1546] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1547]
    var y = a[-1548]
    a[1547] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1548]
    var y = a[-1549]
    a[1548] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1549]
    var y = a[-1550]
    a[1549] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1550]
    var y = a[-1551]
    a[1550] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1551]
    var y = a[-1552]
    a[1551] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1552]
    var y = a[-1553]
    a[1552] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1553]
    var y = a[-1554]
    a[1553] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1554]
    var y = a[-1555]
    a[1554] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1555]
    var y = a[-1556]
    a[1555] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1556]
    var y = a[-1557]
    a[1556] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1557]
    var y = a[-1558]
    a[1557] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1558]
    var y = a[-1559]
    a[1558] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1559]
    var y = a[-1560]
    a[1559] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1560]
    var y = a[-1561]
    a[1560] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1561]
    var y = a[-1562]
    a[1561] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1562]
    var y = a[-1563]
    a[1562] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1563]
    var y = a[-1564]
    a[1563] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1564]
    var y = a[-1565]
    a[1564] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1565]
    var y = a[-1566]
    a[1565] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1566]
    var y = a[-1567]
    a[1566] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1567]
    var y = a[-1568]
    a[1567] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1568]
    var y = a[-1569]
    a[1568] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1569]
    var y = a[-1570]
    a[1569] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1570]
    var y = a[-1571]
    a[1570] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1571]
    var y = a[-1572]
    a[1571] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1572]
    var y = a[-1573]
    a[1572] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1573]
    var y = a[-1574]
    a[1573] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1574]
    var y = a[-1575]
    a[1574] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1575]
    var y = a[-1576]
    a[1575] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1576]
    var y = a[-1577]
    a[1576] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1577]
    var y = a[-1578]
    a[1577] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1578]
    var y = a[-1579]
    a[1578] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1579]
    var y = a[-1580]
    a[1579] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1580]
    var y = a[-1581]
    a[1580] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1581]
    var y = a[-1582]
    a[1581] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1582]
    var y = a[-1583]
    a[1582] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1583]
    var y = a[-1584]
    a[1583] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1584]
    var y = a[-1585]
    a[1584] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1585]
    var y = a[-1586]
    a[1585] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1586]
    var y = a[-1587]
    a[1586] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1587]
    var y = a[-1588]
    a[1587] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1588]
    var y = a[-1589]
    a[1588] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1589]
    var y = a[-1590]
    a[1589] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1590]
    var y = a[-1591]
    a[1590] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1591]
    var y = a[-1592]
    a[1591] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1592]
    var y = a[-1593]
    a[1592] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1593]
    var y = a[-1594]
    a[1593] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1594]
    var y = a[-1595]
    a[1594] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1595]
    var y = a[-1596]
    a[1595] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1596]
    var y = a[-1597]
    a[1596] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1597]
    var y = a[-1598]
    a[1597] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1598]
    var y = a[-1599]
    a[1598] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1599]
    var y = a[-1600]
    a[1599] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1600]
    var y = a[-1601]
    a[1600] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1601]
    var y = a[-1602]
    a[1601] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1602]
    var y = a[-1603]
    a[1602] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1603]
    var y = a[-1604]
    a[1603] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1604]
    var y = a[-1605]
    a[1604] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1605]
    var y = a[-1606]
    a[1605] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1606]
    var y = a[-1607]
    a[1606] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1607]
    var y = a[-1608]
    a[1607] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1608]
    var y = a[-1609]
    a[1608] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1609]
    var y = a[-1610]
    a[1609] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1610]
    var y = a[-1611]
    a[1610] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1611]
    var y = a[-1612]
    a[1611] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1612]
    var y = a[-1613]
    a[1612] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1613]
    var y = a[-1614]
    a[1613] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1614]
    var y = a[-1615]
    a[1614] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1615]
    var y = a[-1616]
    a[1615] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1616]
    var y = a[-1617]
    a[1616] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1617]
    var y = a[-1618]
    a[1617] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1618]
    var y = a[-1619]
    a[1618] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1619]
    var y = a[-1620]
    a[1619] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1620]
    var y = a[-1621]
    a[1620] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1621]
    var y = a[-1622]
    a[1621] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1622]
    var y = a[-1623]
    a[1622] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1623]
    var y = a[-1624]
    a[1623] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1624]
    var y = a[-1625]
    a[1624] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1625]
    var y = a[-1626]
    a[1625] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1626]
    var y = a[-1627]
    a[1626] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1627]
    var y = a[-1628]
    a[1627] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1628]
    var y = a[-1629]
    a[1628] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1629]
    var y = a[-1630]
    a[1629] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1630]
    var y = a[-1631]
    a[1630] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1631]
    var y = a[-1632]
    a[1631] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1632]
    var y = a[-1633]
    a[1632] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1633]
    var y = a[-1634]
    a[1633] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1634]
    var y = a[-1635]
    a[1634] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1635]
    var y = a[-1636]
    a[1635] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1636]
    var y = a[-1637]
    a[1636] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1637]
    var y = a[-1638]
    a[1637] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1638]
    var y = a[-1639]
    a[1638] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1639]
    var y = a[-1640]
    a[1639] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1640]
    var y = a[-1641]
    a[1640] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1641]
    var y = a[-1642]
    a[1641] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1642]
    var y = a[-1643]
    a[1642] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1643]
    var y = a[-1644]
    a[1643] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1644]
    var y = a[-1645]
    a[1644] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1645]
    var y = a[-1646]
    a[1645] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1646]
    var y = a[-1647]
    a[1646] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1647]
    var y = a[-1648]
    a[1647] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1648]
    var y = a[-1649]
    a[1648] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1649]
    var y = a[-1650]
    a[1649] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1650]
    var y = a[-1651]
    a[1650] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1651]
    var y = a[-1652]
    a[1651] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1652]
    var y = a[-1653]
    a[1652] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1653]
    var y = a[-1654]
    a[1653] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1654]
    var y = a[-1655]
    a[1654] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1655]
    var y = a[-1656]
    a[1655] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1656]
    var y = a[-1657]
    a[1656] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1657]
    var y = a[-1658]
    a[1657] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1658]
    var y = a[-1659]
    a[1658] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1659]
    var y = a[-1660]
    a[1659] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1660]
    var y = a[-1661]
    a[1660] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1661]
    var y = a[-1662]
    a[1661] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1662]
    var y = a[-1663]
    a[1662] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1663]
    var y = a[-1664]
    a[1663] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1664]
    var y = a[-1665]
    a[1664] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1665]
    var y = a[-1666]
    a[1665] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1666]
    var y = a[-1667]
    a[1666] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1667]
    var y = a[-1668]
    a[1667] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1668]
    var y = a[-1669]
    a[1668] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1669]
    var y = a[-1670]
    a[1669] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1670]
    var y = a[-1671]
    a[1670] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1671]
    var y = a[-1672]
    a[1671] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1672]
    var y = a[-1673]
    a[1672] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1673]
    var y = a[-1674]
    a[1673] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1674]
    var y = a[-1675]
    a[1674] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1675]
    var y = a[-1676]
    a[1675] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1676]
    var y = a[-1677]
    a[1676] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1677]
    var y = a[-1678]
    a[1677] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1678]
    var y = a[-1679]
    a[1678] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1679]
    var y = a[-1680]
    a[1679] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1680]
    var y = a[-1681]
    a[1680] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1681]
    var y = a[-1682]
    a[1681] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1682]
    var y = a[-1683]
    a[1682] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1683]
    var y = a[-1684]
    a[1683] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1684]
    var y = a[-1685]
    a[1684] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1685]
    var y = a[-1686]
    a[1685] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1686]
    var y = a[-1687]
    a[1686] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1687]
    var y = a[-1688]
    a[1687] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1688]
    var y = a[-1689]
    a[1688] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1689]
    var y = a[-1690]
    a[1689] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1690]
    var y = a[-1691]
    a[1690] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1691]
    var y = a[-1692]
    a[1691] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1692]
    var y = a[-1693]
    a[1692] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1693]
    var y = a[-1694]
    a[1693] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1694]
    var y = a[-1695]
    a[1694] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1695]
    var y = a[-1696]
    a[1695] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1696]
    var y = a[-1697]
    a[1696] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1697]
    var y = a[-1698]
    a[1697] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1698]
    var y = a[-1699]
    a[1698] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1699]
    var y = a[-1700]
    a[1699] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1700]
    var y = a[-1701]
    a[1700] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1701]
    var y = a[-1702]
    a[1701] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1702]
    var y = a[-1703]
    a[1702] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1703]
    var y = a[-1704]
    a[1703] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1704]
    var y = a[-1705]
    a[1704] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1705]
    var y = a[-1706]
    a[1705] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1706]
    var y = a[-1707]
    a[1706] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1707]
    var y = a[-1708]
    a[1707] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1708]
    var y = a[-1709]
    a[1708] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1709]
    var y = a[-1710]
    a[1709] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1710]
    var y = a[-1711]
    a[1710] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1711]
    var y = a[-1712]
    a[1711] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1712]
    var y = a[-1713]
    a[1712] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1713]
    var y = a[-1714]
    a[1713] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1714]
    var y = a[-1715]
    a[1714] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1715]
    var y = a[-1716]
    a[1715] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1716]
    var y = a[-1717]
    a[1716] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1717]
    var y = a[-1718]
    a[1717] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1718]
    var y = a[-1719]
    a[1718] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1719]
    var y = a[-1720]
    a[1719] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1720]
    var y = a[-1721]
    a[1720] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1721]
    var y = a[-1722]
    a[1721] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1722]
    var y = a[-1723]
    a[1722] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1723]
    var y = a[-1724]
    a[1723] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1724]
    var y = a[-1725]
    a[1724] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1725]
    var y = a[-1726]
    a[1725] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1726]
    var y = a[-1727]
    a[1726] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1727]
    var y = a[-1728]
    a[1727] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1728]
    var y = a[-1729]
    a[1728] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1729]
    var y = a[-1730]
    a[1729] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1730]
    var y = a[-1731]
    a[1730] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1731]
    var y = a[-1732]
    a[1731] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1732]
    var y = a[-1733]
    a[1732] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1733]
    var y = a[-1734]
    a[1733] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1734]
    var y = a[-1735]
    a[1734] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1735]
    var y = a[-1736]
    a[1735] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1736]
    var y = a[-1737]
    a[1736] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1737]
    var y = a[-1738]
    a[1737] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1738]
    var y = a[-1739]
    a[1738] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1739]
    var y = a[-1740]
    a[1739] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1740]
    var y = a[-1741]
    a[1740] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1741]
    var y = a[-1742]
    a[1741] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1742]
    var y = a[-1743]
    a[1742] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1743]
    var y = a[-1744]
    a[1743] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1744]
    var y = a[-1745]
    a[1744] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1745]
    var y = a[-1746]
    a[1745] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1746]
    var y = a[-1747]
    a[1746] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1747]
    var y = a[-1748]
    a[1747] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1748]
    var y = a[-1749]
    a[1748] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1749]
    var y = a[-1750]
    a[1749] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1750]
    var y = a[-1751]
    a[1750] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1751]
    var y = a[-1752]
    a[1751] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1752]
    var y = a[-1753]
    a[1752] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1753]
    var y = a[-1754]
    a[1753] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1754]
    var y = a[-1755]
    a[1754] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1755]
    var y = a[-1756]
    a[1755] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1756]
    var y = a[-1757]
    a[1756] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1757]
    var y = a[-1758]
    a[1757] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1758]
    var y = a[-1759]
    a[1758] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1759]
    var y = a[-1760]
    a[1759] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1760]
    var y = a[-1761]
    a[1760] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1761]
    var y = a[-1762]
    a[1761] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1762]
    var y = a[-1763]
    a[1762] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1763]
    var y = a[-1764]
    a[1763] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1764]
    var y = a[-1765]
    a[1764] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1765]
    var y = a[-1766]
    a[1765] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1766]
    var y = a[-1767]
    a[1766] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1767]
    var y = a[-1768]
    a[1767] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1768]
    var y = a[-1769]
    a[1768] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1769]
    var y = a[-1770]
    a[1769] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1770]
    var y = a[-1771]
    a[1770] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1771]
    var y = a[-1772]
    a[1771] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1772]
    var y = a[-1773]
    a[1772] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1773]
    var y = a[-1774]
    a[1773] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1774]
    var y = a[-1775]
    a[1774] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1775]
    var y = a[-1776]
    a[1775] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1776]
    var y = a[-1777]
    a[1776] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1777]
    var y = a[-1778]
    a[1777] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1778]
    var y = a[-1779]
    a[1778] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1779]
    var y = a[-1780]
    a[1779] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1780]
    var y = a[-1781]
    a[1780] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1781]
    var y = a[-1782]
    a[1781] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1782]
    var y = a[-1783]
    a[1782] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1783]
    var y = a[-1784]
    a[1783] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1784]
    var y = a[-1785]
    a[1784] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1785]
    var y = a[-1786]
    a[1785] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1786]
    var y = a[-1787]
    a[1786] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1787]
    var y = a[-1788]
    a[1787] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1788]
    var y = a[-1789]
    a[1788] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1789]
    var y = a[-1790]
    a[1789] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1790]
    var y = a[-1791]
    a[1790] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1791]
    var y = a[-1792]
    a[1791] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1792]
    var y = a[-1793]
    a[1792] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1793]
    var y = a[-1794]
    a[1793] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1794]
    var y = a[-1795]
    a[1794] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1795]
    var y = a[-1796]
    a[1795] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1796]
    var y = a[-1797]
    a[1796] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1797]
    var y = a[-1798]
    a[1797] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1798]
    var y = a[-1799]
    a[1798] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1799]
    var y = a[-1800]
    a[1799] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1800]
    var y = a[-1801]
    a[1800] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1801]
    var y = a[-1802]
    a[1801] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1802]
    var y = a[-1803]
    a[1802] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1803]
    var y = a[-1804]
    a[1803] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1804]
    var y = a[-1805]
    a[1804] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1805]
    var y = a[-1806]
    a[1805] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1806]
    var y = a[-1807]
    a[1806] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1807]
    var y = a[-1808]
    a[1807] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1808]
    var y = a[-1809]
    a[1808] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1809]
    var y = a[-1810]
    a[1809] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1810]
    var y = a[-1811]
    a[1810] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1811]
    var y = a[-1812]
    a[1811] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1812]
    var y = a[-1813]
    a[1812] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1813]
    var y = a[-1814]
    a[1813] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1814]
    var y = a[-1815]
    a[1814] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1815]
    var y = a[-1816]
    a[1815] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1816]
    var y = a[-1817]
    a[1816] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1817]
    var y = a[-1818]
    a[1817] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1818]
    var y = a[-1819]
    a[1818] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1819]
    var y = a[-1820]
    a[1819] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1820]
    var y = a[-1821]
    a[1820] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1821]
    var y = a[-1822]
    a[1821] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1822]
    var y = a[-1823]
    a[1822] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1823]
    var y = a[-1824]
    a[1823] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1824]
    var y = a[-1825]
    a[1824] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1825]
    var y = a[-1826]
    a[1825] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1826]
    var y = a[-1827]
    a[1826] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1827]
    var y = a[-1828]
    a[1827] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1828]
    var y = a[-1829]
    a[1828] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1829]
    var y = a[-1830]
    a[1829] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1830]
    var y = a[-1831]
    a[1830] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1831]
    var y = a[-1832]
    a[1831] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1832]
    var y = a[-1833]
    a[1832] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1833]
    var y = a[-1834]
    a[1833] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1834]
    var y = a[-1835]
    a[1834] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1835]
    var y = a[-1836]
    a[1835] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1836]
    var y = a[-1837]
    a[1836] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1837]
    var y = a[-1838]
    a[1837] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1838]
    var y = a[-1839]
    a[1838] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1839]
    var y = a[-1840]
    a[1839] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1840]
    var y = a[-1841]
    a[1840] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1841]
    var y = a[-1842]
    a[1841] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1842]
    var y = a[-1843]
    a[1842] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1843]
    var y = a[-1844]
    a[1843] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1844]
    var y = a[-1845]
    a[1844] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1845]
    var y = a[-1846]
    a[1845] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1846]
    var y = a[-1847]
    a[1846] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1847]
    var y = a[-1848]
    a[1847] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1848]
    var y = a[-1849]
    a[1848] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1849]
    var y = a[-1850]
    a[1849] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1850]
    var y = a[-1851]
    a[1850] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1851]
    var y = a[-1852]
    a[1851] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1852]
    var y = a[-1853]
    a[1852] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1853]
    var y = a[-1854]
    a[1853] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1854]
    var y = a[-1855]
    a[1854] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1855]
    var y = a[-1856]
    a[1855] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1856]
    var y = a[-1857]
    a[1856] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1857]
    var y = a[-1858]
    a[1857] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1858]
    var y = a[-1859]
    a[1858] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1859]
    var y = a[-1860]
    a[1859] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1860]
    var y = a[-1861]
    a[1860] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1861]
    var y = a[-1862]
    a[1861] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1862]
    var y = a[-1863]
    a[1862] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1863]
    var y = a[-1864]
    a[1863] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1864]
    var y = a[-1865]
    a[1864] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1865]
    var y = a[-1866]
    a[1865] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1866]
    var y = a[-1867]
    a[1866] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1867]
    var y = a[-1868]
    a[1867] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1868]
    var y = a[-1869]
    a[1868] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1869]
    var y = a[-1870]
    a[1869] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1870]
    var y = a[-1871]
    a[1870] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1871]
    var y = a[-1872]
    a[1871] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1872]
    var y = a[-1873]
    a[1872] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1873]
    var y = a[-1874]
    a[1873] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1874]
    var y = a[-1875]
    a[1874] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1875]
    var y = a[-1876]
    a[1875] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1876]
    var y = a[-1877]
    a[1876] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1877]
    var y = a[-1878]
    a[1877] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1878]
    var y = a[-1879]
    a[1878] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1879]
    var y = a[-1880]
    a[1879] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1880]
    var y = a[-1881]
    a[1880] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1881]
    var y = a[-1882]
    a[1881] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1882]
    var y = a[-1883]
    a[1882] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1883]
    var y = a[-1884]
    a[1883] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1884]
    var y = a[-1885]
    a[1884] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1885]
    var y = a[-1886]
    a[1885] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1886]
    var y = a[-1887]
    a[1886] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1887]
    var y = a[-1888]
    a[1887] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1888]
    var y = a[-1889]
    a[1888] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1889]
    var y = a[-1890]
    a[1889] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1890]
    var y = a[-1891]
    a[1890] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1891]
    var y = a[-1892]
    a[1891] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1892]
    var y = a[-1893]
    a[1892] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1893]
    var y = a[-1894]
    a[1893] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1894]
    var y = a[-1895]
    a[1894] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1895]
    var y = a[-1896]
    a[1895] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1896]
    var y = a[-1897]
    a[1896] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1897]
    var y = a[-1898]
    a[1897] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1898]
    var y = a[-1899]
    a[1898] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1899]
    var y = a[-1900]
    a[1899] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1900]
    var y = a[-1901]
    a[1900] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1901]
    var y = a[-1902]
    a[1901] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1902]
    var y = a[-1903]
    a[1902] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1903]
    var y = a[-1904]
    a[1903] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1904]
    var y = a[-1905]
    a[1904] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1905]
    var y = a[-1906]
    a[1905] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1906]
    var y = a[-1907]
    a[1906] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1907]
    var y = a[-1908]
    a[1907] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1908]
    var y = a[-1909]
    a[1908] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1909]
    var y = a[-1910]
    a[1909] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1910]
    var y = a[-1911]
    a[1910] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1911]
    var y = a[-1912]
    a[1911] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1912]
    var y = a[-1913]
    a[1912] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1913]
    var y = a[-1914]
    a[1913] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1914]
    var y = a[-1915]
    a[1914] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1915]
    var y = a[-1916]
    a[1915] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1916]
    var y = a[-1917]
    a[1916] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1917]
    var y = a[-1918]
    a[1917] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1918]
    var y = a[-1919]
    a[1918] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1919]
    var y = a[-1920]
    a[1919] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1920]
    var y = a[-1921]
    a[1920] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1921]
    var y = a[-1922]
    a[1921] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1922]
    var y = a[-1923]
    a[1922] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1923]
    var y = a[-1924]
    a[1923] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1924]
    var y = a[-1925]
    a[1924] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1925]
    var y = a[-1926]
    a[1925] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1926]
    var y = a[-1927]
    a[1926] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1927]
    var y = a[-1928]
    a[1927] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1928]
    var y = a[-1929]
    a[1928] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1929]
    var y = a[-1930]
    a[1929] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1930]
    var y = a[-1931]
    a[1930] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1931]
    var y = a[-1932]
    a[1931] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1932]
    var y = a[-1933]
    a[1932] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1933]
    var y = a[-1934]
    a[1933] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1934]
    var y = a[-1935]
    a[1934] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1935]
    var y = a[-1936]
    a[1935] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1936]
    var y = a[-1937]
    a[1936] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1937]
    var y = a[-1938]
    a[1937] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1938]
    var y = a[-1939]
    a[1938] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1939]
    var y = a[-1940]
    a[1939] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1940]
    var y = a[-1941]
    a[1940] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1941]
    var y = a[-1942]
    a[1941] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1942]
    var y = a[-1943]
    a[1942] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1943]
    var y = a[-1944]
    a[1943] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1944]
    var y = a[-1945]
    a[1944] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1945]
    var y = a[-1946]
    a[1945] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1946]
    var y = a[-1947]
    a[1946] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1947]
    var y = a[-1948]
    a[1947] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1948]
    var y = a[-1949]
    a[1948] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1949]
    var y = a[-1950]
    a[1949] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1950]
    var y = a[-1951]
    a[1950] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1951]
    var y = a[-1952]
    a[1951] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1952]
    var y = a[-1953]
    a[1952] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1953]
    var y = a[-1954]
    a[1953] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1954]
    var y = a[-1955]
    a[1954] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1955]
    var y = a[-1956]
    a[1955] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1956]
    var y = a[-1957]
    a[1956] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1957]
    var y = a[-1958]
    a[1957] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1958]
    var y = a[-1959]
    a[1958] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1959]
    var y = a[-1960]
    a[1959] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1960]
    var y = a[-1961]
    a[1960] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1961]
    var y = a[-1962]
    a[1961] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1962]
    var y = a[-1963]
    a[1962] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1963]
    var y = a[-1964]
    a[1963] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1964]
    var y = a[-1965]
    a[1964] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1965]
    var y = a[-1966]
    a[1965] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1966]
    var y = a[-1967]
    a[1966] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1967]
    var y = a[-1968]
    a[1967] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1968]
    var y = a[-1969]
    a[1968] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1969]
    var y = a[-1970]
    a[1969] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1970]
    var y = a[-1971]
    a[1970] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1971]
    var y = a[-1972]
    a[1971] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1972]
    var y = a[-1973]
    a[1972] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1973]
    var y = a[-1974]
    a[1973] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1974]
    var y = a[-1975]
    a[1974] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1975]
    var y = a[-1976]
    a[1975] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1976]
    var y = a[-1977]
    a[1976] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1977]
    var y = a[-1978]
    a[1977] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1978]
    var y = a[-1979]
    a[1978] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1979]
    var y = a[-1980]
    a[1979] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1980]
    var y = a[-1981]
    a[1980] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1981]
    var y = a[-1982]
    a[1981] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1982]
    var y = a[-1983]
    a[1982] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1983]
    var y = a[-1984]
    a[1983] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1984]
    var y = a[-1985]
    a[1984] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1985]
    var y = a[-1986]
    a[1985] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1986]
    var y = a[-1987]
    a[1986] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1987]
    var y = a[-1988]
    a[1987] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1988]
    var y = a[-1989]
    a[1988] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1989]
    var y = a[-1990]
    a[1989] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1990]
    var y = a[-1991]
    a[1990] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1991]
    var y = a[-1992]
    a[1991] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1992]
    var y = a[-1993]
    a[1992] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1993]
    var y = a[-1994]
    a[1993] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1994]
    var y = a[-1995]
    a[1994] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1995]
    var y = a[-1996]
    a[1995] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1996]
    var y = a[-1997]
    a[1996] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1997]
    var y = a[-1998]
    a[1997] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1998]
    var y = a[-1999]
    a[1998] = math.round(math.sqrt(x * x - y * y))
  },
  @(a) {
    var x = a[1999]
    var y = a[-2000]
    a[1999] = math.round(math.sqrt(x * x - y * y))
  },
]

def setup() {
  var a = []
  for i in 0..2000 {
    a.append(i)
  }
  return a
}

var array = setup()

def run() {
  var run_times = 10000
  for time in 0..run_times {
    for j in array {
      funcs[j](array)
    }
  }
}

var start = microtime()
run()
var end = microtime()

echo 'Time taken = ${(end - start) / 1000000} seconds'

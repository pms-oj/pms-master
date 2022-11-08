#include "prize.h"
#include <map>
#include <vector>
#include <cassert>

using namespace std;

const int MAX_NAIVE = 480;

map<int, vector<int> > cache;

vector<int> ask_cached(int i) {
	if (cache.find(i) != cache.end()) {
		return cache[i];
	}
	auto r = ask(i);
	cache[i] = r;
	return r;
}

int find_best(int n) {
	int max_sum = 0;
	int s_idx = 0;
	auto s_q = vector<int>();
	for(int i = 0; i < min(MAX_NAIVE, n); i++) {
		vector<int> res = ask_cached(i);
		if (max_sum <= res[0] + res[1]) {
			max_sum = max(max_sum, res[0] + res[1]);
			s_idx = i;
			s_q = res;
		}
	}
	vector<int> nonlollipop;
	for (int i = 0; i < min(MAX_NAIVE, n); i++) {
		vector<int> res = ask_cached(i);
		if (max_sum > res[0] + res[1]) {
			nonlollipop.push_back(i);
		}
	}
	int e_idx = n-1;
	auto e_q = s_q;
	while (e_idx > s_idx) {
		vector<int> res = ask_cached(e_idx);
		if (max_sum == res[0] + res[1]) {
			e_q = res;
			break;
		} else {
			nonlollipop.push_back(e_idx);
		}
		e_idx--;
	}
	while (e_q[0] - s_q[0] > 0 && s_idx < e_idx) {
		int l = s_idx, r = e_idx;
		while (l < r) {
			int mid = (l+r)/2;
			auto q = ask_cached(mid);
			if (q[0] + q[1] == max_sum) {
			if (q[0] - s_q[0] == 0) {
				l = mid+1;
			} else {
				r = mid;
			}
			} else {
				r = mid;
			}
		}
		if (l < e_idx && l > s_idx) {
			auto q = ask_cached(l);
			if (q[0] + q[1] != max_sum) {
				s_q[0]++;
				s_q[1]--;
				nonlollipop.push_back(l);
				s_idx = l;
			} else {
				break;
			}
		} else {
			break;
		}
	}
	int val = 0;
	for (auto i: nonlollipop) {
		vector<int> res = ask_cached(i);
		if (res[0] + res[1] == 0) {
			val = i;
		}
	}
	return val;
}

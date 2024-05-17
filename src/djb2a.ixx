module;
#include "typedefs.hh"
#include <string>
#include <string_view>
export module djb2a;

export
{
	inline constexpr u32 hash_djb2a(const std::string_view sv)
	{
		u32 hash{ 5381 };
		for (unsigned char c : sv) {
			hash = ((hash << 5) + hash) ^ c;
		}
		return hash;
	}

	inline constexpr auto operator"" _sh(const char *str, size_t len)
	{
		return hash_djb2a(std::string_view{ str, len });
	}
}

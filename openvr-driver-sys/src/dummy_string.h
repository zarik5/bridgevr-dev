#pragma once

namespace std
{
class dummy_string
{
public:
    dummy_string();
    dummy_string(const char *);
    const char *c_str() const;
};
} // namespace std